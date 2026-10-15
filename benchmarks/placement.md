# Compare punctuation code placements

Use this experiment to separate code placement effects from scanner changes.
The extraction supports Rust assembly for Windows x64 MSVC. Node.js generates
the experiment. Rust builds it. PowerShell sets the processor affinity.

The probe uses the existing punctuation definition and source. It has three
callers: token count, total span length, and token values. These callers retain
different scanner results during optimization. Each call processes 524,475
bytes with the current source.

## Prepare each revision

1. Put each revision in a separate source directory.
2. Copy `benches/punctuation_layout.rs` to each benchmark workspace.
3. Add its `[[bench]]` entry from `Cargo.toml` to each workspace.
4. Set `CARGO_TARGET_DIR` to a separate directory under `%TEMP%`.
5. Build the probe with the same Rust version and benchmark profile.

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'runeweaver-placement-base'
cargo rustc --manifest-path benchmarks/Cargo.toml --bench punctuation_layout `
    --profile bench --locked --offline -- --emit=asm,link
```

Use the same benchmark source and locked dependencies for both revisions.
The probe also compiles against `v0.2.0`. Keep that revision's shared scan
runner, which uses its earlier trait signatures.

Find the emitted assembly file whose name contains
`.punctuation_layout.` and ends with `-cgu.0.rcgu.s`. Copy it to the experiment
directory as `base.s`. Execute the original probe once. Save its JSON output
as `base-oracle.json` with UTF-8 encoding and no byte order mark.
Repeat these steps for the candidate as `tip.s` and `tip-oracle.json`.

Create `comparison.json` in that directory:

```json
{
  "input": "C:/path/to/runeweaver/benchmarks/sources/punctuation.txt",
  "revisions": [
    { "name": "base", "assembly": "base.s", "oracle": "base-oracle.json" },
    { "name": "control", "assembly": "base.s", "oracle": "base-oracle.json" },
    { "name": "tip", "assembly": "tip.s", "oracle": "tip-oracle.json" }
  ]
}
```

Paths in the configuration are relative to that file. The control copies the
base instructions to another location with the same boundary offset.
It measures residual variation within the experiment. Also repeat the same
executable to check variation between measurement periods.

## Build the experiment

```powershell
node benchmarks/placement.mjs generate C:/experiment/comparison.json C:/experiment/generated
rustc --edition=2024 -C opt-level=3 C:/experiment/generated/placement.rs `
    -o C:/experiment/placement.exe
```

Read `generated/audit.json` before you compile the generated probe.
It records each original function body, referenced table, replaced symbol,
and assembly hash. It also records the source hash.

The generator retains instructions, registers, branches, stack accesses, and
internal alignment directives. It copies referenced tables recursively.
Each table starts on a 64-byte boundary. Each function starts at offset 0,
16, 32, or 48 within a 64-byte boundary.

The isolated copies abort on scan errors and panics. They have no unwind
metadata. Use only the supplied valid source. The generator rejects unknown
direct calls and indirect calls. It does not support retained scanner helpers.
Use workspace tests to check error paths and complete scanner correctness.

## Measure the experiment

Finish all builds and tests before you measure the executable.
Do not run builds, tests, or another benchmark during measurement.

```powershell
./benchmarks/placement-run.ps1 -Executable C:/experiment/placement.exe `
    -Log C:/experiment/first.log -Cpu 2
./benchmarks/placement-run.ps1 -Executable C:/experiment/placement.exe `
    -Log C:/experiment/second.log -Cpu 2 -ReverseOrder
node benchmarks/placement.mjs summarize C:/experiment/first.log
node benchmarks/placement.mjs summarize C:/experiment/second.log
```

Before timing, the executable checks every linked address and each result
against the original probe. It prints the addresses in the log.
Eight rounds reverse the order in pairs and rotate the first variant.
Each measurement uses ten warmup calls and 200 timed calls by default.
The timed loop contains the scan call and `black_box`. It has no result check.

The summary rejects missing addresses, incorrect offsets, duplicate samples,
missing samples, and incomplete executions. It reports nanoseconds per scan.
For each caller, it reports the best, median, and worst placement medians.
The span is `100 * (worst / best - 1)`.

Report all placements and both measurement periods. Do not treat these four
offsets as a random sample of all executable layouts. They control one source
of variation. They do not remove frequency changes, prediction effects, or
variation from addresses beyond the measured boundary.

Compare branch structure, registers, spills, padding, linked addresses, and
referenced tables before you accept a scanner change. Fewer instructions do
not establish a performance improvement. Check other scan cases before you
retain a production change.
