//! Renders completed selector control flow as Rust source.

use crate::ir::{
    Argument, Block, Call, Context, Dispatch, Function, FunctionBody, MatchValue, Position,
    Selector, Transfer, Width,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(super) fn emit(selector: &Selector) -> TokenStream {
    let starts = selector.starts.iter().enumerate().map(|(mode, start)| {
        let call = call(start);
        quote! { #mode => #call, }
    });
    let functions = selector
        .functions
        .iter()
        .enumerate()
        .map(|(id, plan)| function(id, plan));
    quote! {
        fn __runeweaver_select(input: &[u8], start_condition: usize) -> Option<(::core::num::NonZeroUsize, usize)> {
            match start_condition {
                #(#starts)*
                _ => None,
            }
        }
        #(#functions)*
    }
}

fn function(id: usize, plan: &Function) -> TokenStream {
    let name = format_ident!("__runeweaver_region_{id}");
    let context_mut = plan.mutable_context.then(|| quote!(mut));
    let context_parameter = match plan.context {
        Context::Empty => quote!(),
        Context::Fixed {
            optional: false, ..
        } => quote!(, #context_mut saved_end: usize),
        Context::Fixed { optional: true, .. } => quote!(, #context_mut saved_end: Option<usize>),
        Context::General => {
            quote!(, #context_mut latest: Option<(::core::num::NonZeroUsize, usize)>)
        }
    };
    let index_mut = plan.mutable_index.then(|| quote!(mut));
    let entry_parameter = plan.entry_parameter.then(|| quote!(, mut state: usize));
    let body = match &plan.body {
        FunctionBody::Linear(code) => block(code),
        FunctionBody::Loop(code) => {
            let code = block(code);
            quote! { 'region: loop { #code } }
        }
        FunctionBody::Dispatch { initial, arms } => {
            let init = initial.map(|state| quote!(let mut state = #state;));
            let arms = arms.iter().enumerate().map(|(ordinal, code)| {
                let code = block(code);
                quote! { #ordinal => { #code }, }
            });
            quote! {
                #init
                'region: loop {
                    match state {
                        #(#arms)*
                        _ => unreachable!("generated region selected an unknown state"),
                    }
                }
            }
        }
    };
    quote! {
        fn #name(input: &[u8], #index_mut index: usize #context_parameter #entry_parameter)
            -> Option<(::core::num::NonZeroUsize, usize)>
        {
            #body
        }
    }
}

fn call(plan: &Call) -> TokenStream {
    let name = format_ident!("__runeweaver_region_{}", plan.region);
    let index = position(plan.index);
    let context = match plan.context {
        Argument::Omitted => quote!(),
        Argument::ZeroEnd => quote!(, 0),
        Argument::EmptyEnd | Argument::EmptyMatch => quote!(, None),
        Argument::RequiredEnd(value) => {
            let saved = match_value(value);
            quote!(, (#saved).expect("the saved rule is live").1)
        }
        Argument::OptionalEnd(value) => {
            let saved = match_value(value);
            quote!(, (#saved).map(|(_, end)| end))
        }
        Argument::Match(value) => {
            let saved = match_value(value);
            quote!(, #saved)
        }
    };
    let entry = plan.entry.map(|ordinal| quote!(, #ordinal));
    quote!(Self::#name(input, #index #context #entry))
}

fn block(plan: &Block) -> TokenStream {
    let (self_loop, failure, dispatch) = match plan {
        Block::Return(value) => {
            let value = match_value(*value);
            return quote!(return #value;);
        }
        Block::Scan {
            self_loop,
            failure,
            dispatch,
        } => (self_loop, failure, dispatch),
    };
    let failure = match_value(*failure);
    let fast_loop = self_loop.as_ref().map(|plan| {
        let table = plan.values.iter();
        let chunk_size = proc_macro2::Literal::usize_unsuffixed(plan.chunk_size);
        let tests = plan.probes.iter().map(|(offset, next)| {
            quote! {
                if SELF_LOOP[bytes[#offset] as usize] & SELF_LOOP[bytes[#next] as usize] == 0 {
                    index += #offset;
                    break 'fast;
                }
            }
        });
        quote! {
            const SELF_LOOP: &[u8; 256] = &[#(#table as u8),*];
            'fast: while index + #chunk_size <= input.len() {
                let bytes: &[u8; #chunk_size] = input[index..index + #chunk_size]
                    .try_into().expect("an eight-byte slice has eight bytes");
                #(#tests)*
                index += #chunk_size;
            }
            while index < input.len() && SELF_LOOP[input[index] as usize] != 0 {
                index += 1;
            }
        }
    });
    let (read, fork) = match dispatch {
        Dispatch::Fail => (quote!(), quote!(return #failure;)),
        Dispatch::Table {
            width: cell,
            values,
            branches,
        } => {
            let width = width(*cell);
            let table = values.iter().map(|ordinal| quote!(#ordinal as #width));
            let branches = branches.iter().enumerate().map(|(ordinal, plan)| {
                let ordinal = proc_macro2::Literal::usize_unsuffixed(ordinal);
                let code = transfer(plan);
                quote!(#ordinal => { #code },)
            });
            (
                quote!(let byte = input[index];),
                quote! {
                    const FORK: &[#width; 256] = &[#(#table),*];
                    match FORK[byte as usize] {
                        #(#branches)*
                        _ => return #failure,
                    }
                },
            )
        }
        Dispatch::Ranges(branches) => {
            let read = quote!(let byte = input[index];);
            let branches = branches.iter().map(|(ranges, plan)| {
                let tests = ranges.iter().map(|range| {
                    let low = range.low;
                    let high = range.high;
                    quote!((#low..=#high).contains(&byte))
                });
                let code = transfer(plan);
                quote! { if false #(|| #tests)* { #code } }
            });
            (quote!(#read), quote! { #(#branches)* return #failure; })
        }
    };
    quote! {
        #fast_loop
        if index == input.len() { return #failure; }
        #read
        #fork
    }
}

fn transfer(plan: &Transfer) -> TokenStream {
    match plan {
        Transfer::Return(value) => {
            let value = match_value(*value);
            quote!(return #value;)
        }
        Transfer::Call(plan) => {
            let call = call(plan);
            quote!(return #call;)
        }
        Transfer::Continue { update, header } => {
            let update = update.map(|(context, value)| {
                let saved = match_value(value);
                match context {
                    Context::Empty => quote!(),
                    Context::Fixed {
                        optional: false, ..
                    } => quote!(saved_end = (#saved).expect("the saved rule is live").1;),
                    Context::Fixed { optional: true, .. } => {
                        quote!(saved_end = (#saved).map(|(_, end)| end);)
                    }
                    Context::General => quote!(latest = #saved;),
                }
            });
            let select = header.map(|ordinal| quote!(state = #ordinal;));
            quote! { #update index += 1; #select continue 'region; }
        }
        Transfer::Inline { bind, body } => {
            let bind = bind.map(|value| {
                let saved = match_value(value);
                quote!(let saved = #saved;)
            });
            let code = block(body);
            quote! { #bind index += 1; #code }
        }
    }
}

fn match_value(value: MatchValue) -> TokenStream {
    match value {
        MatchValue::Empty | MatchValue::Context(Context::Empty) => {
            quote!(None::<(::core::num::NonZeroUsize, usize)>)
        }
        MatchValue::Accepted { rule, end } => {
            let rule = rule_tag(rule);
            let end = position(end);
            quote!(Some((#rule, #end)))
        }
        MatchValue::Context(Context::Fixed {
            rule,
            optional: false,
        }) => {
            let rule = rule_tag(rule);
            quote!(Some((#rule, saved_end)))
        }
        MatchValue::Context(Context::Fixed {
            rule,
            optional: true,
        }) => {
            let rule = rule_tag(rule);
            quote!(saved_end.map(|end| (#rule, end)))
        }
        MatchValue::Context(Context::General) => quote!(latest),
        MatchValue::Local => quote!(saved),
    }
}

fn position(value: Position) -> TokenStream {
    match value {
        Position::Zero => quote!(0),
        Position::Current => quote!(index),
        Position::Next => quote!(index + 1),
    }
}

fn width(value: Width) -> TokenStream {
    match value {
        Width::U8 => quote!(u8),
        Width::U16 => quote!(u16),
        Width::U32 => quote!(u32),
        Width::Usize => quote!(usize),
    }
}

fn rule_tag(rule: usize) -> TokenStream {
    let tag = rule
        .checked_add(1)
        .expect("a generated rule index fits its tag");
    quote!(::core::num::NonZeroUsize::new(#tag).expect("a generated rule tag is nonzero"))
}
