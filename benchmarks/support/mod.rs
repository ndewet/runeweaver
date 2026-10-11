pub mod definition;
pub mod generation;
pub mod scan;

macro_rules! token_definition {
    (
        $(#[$enum_attribute:meta])*
        $visibility:vis enum $token:ident {
            $(
                $(#[$variant_attribute:meta])*
                $variant:ident $(($payload:ty))?
            ),* $(,)?
        }
    ) => {
        #[derive(runeweaver::Lexer)]
        $(#[$enum_attribute])*
        $visibility enum $token {
            $(
                $(#[$variant_attribute])*
                $variant $(($payload))?
            ),*
        }

        pub fn definition() -> $crate::support::generation::Input {
            let _ = std::marker::PhantomData::<$token>;
            $crate::support::definition::parse(stringify!(
                $(#[$enum_attribute])*
                enum $token {
                    $(
                        $(#[$variant_attribute])*
                        $variant $(($payload))?
                    ),*
                }
            ))
        }
    };
}

pub(crate) use token_definition;
