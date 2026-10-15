# Benchmarks

This workspace contains all performance benchmarks for Runeweaver. It stays
separate so benchmark dependencies do not enter the published crates.

Run the complete suite from the repository root:

```console
cargo bench --manifest-path benchmarks/Cargo.toml
```

Criterion warms each case for one second. It then collects 200 samples over at
least three seconds. Reports use a 99% confidence level and a 1%
significance level. Flat sampling keeps the duration predictable for operations
that take several milliseconds.
The benchmark profile uses one code generation unit and thin LTO.
These settings reduce changes caused by compiler partitioning.
The scan function does not inline into Criterion's iteration loop. This keeps
the scanner's optimization context separate from the timing harness. Each call
processes at least 512 KiB, so the call cost is small compared with the scan.

The benchmark timing job is disabled. Machine variation and executable layout
can produce repeatable timing changes without added scanning work.
Thus, these measurements do not provide a reliable pull request gate.
The job configuration stays available for future benchmarking work.

CI still checks benchmark formatting, Clippy, and the minimum supported Rust
version. Local benchmark commands remain available.

For a suspected regression, repeat the comparison and reverse the measurement
order. First measure the same executable against itself to check machine drift.
A narrow Criterion interval describes one measurement period. It does not
exclude changes in CPU frequency, system load, or executable layout between runs.
Compare optimized assembly when an API change has no expected runtime cost.

Compare multiple code placements before you report a scan regression.
Keep the table placement fixed during this comparison. Report each placement,
the median across placements, and the best and worst results. A second
measurement order does not control code placement.

The [punctuation experiment](placement.md) supplies this comparison for three
callers on Windows x64 MSVC. Its isolated copies use offsets 0, 16, 32, and 48
within a 64-byte boundary. These offsets do not change production code.
For other scan cases, use an equivalent placement experiment before you assign
a timing change to the scanner. The punctuation extraction tool rejects
retained helpers and does not support those cases.

Apply benchmark stability changes to both revisions before comparing library
changes. An unchanged library can produce different results with a new harness.
For a stack of pull requests, merge the harness changes first and rebase the stack.

## Add a benchmark

Put each token definition in `cases/`. Use `token_definition!` with the same
attributes and variants that a Runeweaver lexer uses:

```rust
use crate::support::token_definition;

token_definition! {
    #[lexer(skip = r"[ \t\r\n]+")]
    pub enum Token {
        #[token("[a-z]+")]
        Word,
    }
}
```

The macro creates the derived token type and its codegen input. Add a matching
benchmark target to `Cargo.toml`. Add a wrapper in `benches/` that includes
`case_benchmark.rs`. The same form supports modes, payload fields, callbacks,
and mode actions. The corresponding files in `cases/` show each feature in
isolation.

For a scan benchmark, add a source file in `sources/`. Then register the token
type and source with the shared `&str` runner:

```rust
pub const SOURCE: &str = include_str!("../sources/words.txt");

scan::benchmark::<words::Token>(criterion, "words", words::SOURCE);
```

The runner expands small source files to at least 512 KiB and measures the
borrowed string API.

Keep input creation and other setup outside the timed operation. Use
`iter_batched` when each iteration needs a new input value.

Pass inputs and outputs through `std::hint::black_box`. This prevents the
compiler from removing work that the benchmark must measure.

Set Criterion throughput metadata for byte or element processing. Use stable,
descriptive benchmark identifiers because CI uses them to match both revisions.

Use deterministic inputs that represent a documented operation. Make each
iteration do enough work to reduce timer overhead without hiding relevant costs.

The complete suite must compile against the base revision. API changes need
matching benchmark adapters for each revision. Keep the benchmark names stable
when you change those adapters.
