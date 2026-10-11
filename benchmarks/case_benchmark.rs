use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;

fn configuration() -> Criterion {
    Criterion::default()
        .confidence_level(0.99)
        .significance_level(0.01)
        .sample_size(200)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .without_plots()
}

fn benchmarks(criterion: &mut Criterion) {
    let name = env!("CARGO_CRATE_NAME").replace('_', "-");
    support::generation::benchmarks(
        criterion,
        &[support::generation::Definition::new(
            &name,
            case::definition,
        )],
    );
    support::scan::benchmark::<case::Token>(criterion, &name, case::SOURCE);
}

criterion_group! {
    name = benches;
    config = configuration();
    targets = benchmarks
}
criterion_main!(benches);
