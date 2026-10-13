use criterion::{Criterion, SamplingMode, Throughput};
use runeweaver::Lexer;
use runeweaver::lexer::Automaton;
use std::hint::black_box;

const MINIMUM_INPUT_BYTES: usize = 512 * 1024;

fn input(source: &str) -> String {
    assert!(
        !source.is_empty(),
        "a benchmark source file cannot be empty"
    );
    let repetitions = MINIMUM_INPUT_BYTES.div_ceil(source.len());
    source.repeat(repetitions)
}

// Keep the complete scan separate from Criterion's iteration loop.
#[inline(never)]
fn scan<'input, T>(input: &'input str) -> usize
where
    T: Lexer<'input>,
    <T as Automaton<'input>>::Extras: Default,
{
    T::scan(black_box(input))
        .try_fold(0usize, |count, result| result.map(|_| count + 1))
        .expect("the benchmark source is valid for its token definition")
}

pub fn benchmark<T>(criterion: &mut Criterion, name: &str, source: &str)
where
    T: for<'input> Lexer<'input>,
    for<'input> <T as Automaton<'input>>::Extras: Default,
{
    let input = input(source);
    let mut group = criterion.benchmark_group(format!("scanning/{name}"));
    group.sampling_mode(SamplingMode::Flat);
    group.throughput(Throughput::Bytes(input.len() as u64));

    group.bench_function("str", |bencher| {
        bencher.iter(|| black_box(scan::<T>(input.as_str())));
    });

    group.finish();
}
