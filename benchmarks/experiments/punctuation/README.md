# Punctuation placement results

The comparison uses merged main at `a1c6d11` and release `v0.2.0` at
`809711f`. The former stack tip, `26c2afa`, supplies historical evidence.
It is not the current comparison base.

Main subsequently advances to `16200d0` through a README-only change. The
scanner sources and benchmark inputs remain identical to the measured commit.

Code placement reproduces the punctuation timing difference. The current
measurements do not justify a production scanner change.

## Method

The machine uses Windows x64 and an AMD Ryzen 7 9700X. The compiler is
Rust 1.98.0. Builds use the existing benchmark profile with thin LTO and one
code generation unit. All Cargo target directories are under `%TEMP%`.

Each scan processes 524,475 bytes. The original probes return 268,758 tokens,
335,664 matched bytes, and a token-value sum of 4,470,228. Every isolated copy
returns the corresponding result before timing starts.

The experiment uses three callers and four code offsets per caller.
Each referenced table starts on a 64-byte boundary. Each function starts at
offset 0, 16, 32, or 48 within that boundary. Internal alignment directives
remain in the copied assembly. The executable checks every linked offset.

Each measurement has ten warmup calls and 200 timed calls. Eight rounds
alternate and rotate the order. A second period reverses the initial order.
Execution uses logical CPU 2. Builds and tests finish before measurement.

The control copies main's unchanged assembly to another address at each
offset. It detects variation that the four boundary offsets do not explain.

The [first log](first.log) and [second log](second.log) contain all samples
and linked addresses. Use the [placement tool](../../placement.md) to check
and summarize them. These measurements describe this machine and these
callers. They do not establish a portable speed ratio.

## Measurements

Times are microseconds per scan. Each cell gives the first and second
period, separated by a slash. The median column uses the four placement
medians. The range column gives the best and worst placement medians.

| Revision | Caller | Placement median | Placement range |
| --- | --- | ---: | ---: |
| v020 | count | 439.4 / 438.2 | 410.7 to 463.7 / 412.0 to 462.2 |
| v020 | spans | 443.8 / 443.2 | 413.9 to 466.4 / 410.8 to 465.7 |
| v020 | values | 465.7 / 460.3 | 402.1 to 482.3 / 398.1 to 478.2 |
| main | count | 419.3 / 420.1 | 411.3 to 460.7 / 404.3 to 472.0 |
| main | spans | 431.8 / 431.4 | 422.8 to 467.5 / 420.7 to 466.9 |
| main | values | 446.0 / 444.9 | 441.5 to 487.0 / 434.8 to 478.3 |
| control | count | 419.5 / 418.8 | 412.4 to 461.5 / 404.2 to 455.3 |
| control | spans | 429.6 / 424.6 | 420.3 to 468.0 / 414.7 to 464.0 |
| control | values | 448.0 / 443.9 | 439.2 to 483.2 / 436.6 to 480.9 |
| direct | count | 445.9 / 445.9 | 419.5 to 472.8 / 417.8 to 468.7 |
| direct | spans | 445.7 / 444.6 | 419.3 to 470.0 / 413.1 to 470.4 |
| direct | values | 463.4 / 463.7 | 397.3 to 483.4 / 396.6 to 490.6 |
| ranges | count | 443.9 / 446.1 | 416.7 to 455.2 / 412.0 to 453.4 |
| ranges | spans | 440.9 / 442.9 | 414.7 to 463.6 / 414.3 to 460.5 |
| ranges | values | 464.8 / 466.7 | 398.9 to 483.7 / 394.1 to 479.1 |

The [placement medians](placements.csv) list each offset separately. Main's
count caller has a placement spread of 12.0% and 16.7% in these periods.
The direct trial raises its placement median by 6.4% and 6.1%. Its value
caller raises that median by 3.9% and 4.2%, despite a faster offset 32.
The range trial raises the count median by 5.9% and 6.2%.

The control differs from main by less than 1.6% in the caller placement
medians. Individual offsets differ by as much as approximately 3.5%.
This residual variation limits claims about small changes.

## Dispatch trials

The [direct dispatch patch](direct.patch) replaces the ordinal table lookup
with a Rust match on the input byte. The [range patch](ranges.patch) uses
the existing range emitter for all byte dispatches. Apply each patch
separately to main to reproduce its build. Neither patch changes the
production source in this PR.

LLVM lowers both trials to jump tables. The range source does not produce
a branch chain in the measured executable. The count and value callers
have identical trial instructions after symbol normalization. The span
callers place some branch blocks in a different order.

Main loads a byte, loads its ordinal from a 256-byte table, checks the
ordinal, and dispatches through a 23-entry jump table. The trials remove
the ordinal load. They subtract 9 from the byte and check a 117-entry jump
table. The dispatch data grows from 348 to 468 bytes. The whitespace table
remains unchanged.

| Caller | Main instructions | Trial instructions | Branches | Stack reservation | Stack references |
| --- | ---: | ---: | ---: | ---: | ---: |
| Count | 233 | 228 | 53 | 152 bytes | 21 |
| Spans | 237 | 236 | 53 | 120 bytes | 29 |
| Values | 320 | 320 | 80 | 136 bytes | 20 |

Counts include cold paths and explicit NOPs. Branch counts include direct
and indirect jumps. The trials change register allocation without adding
hot-loop spills. For example, the count caller keeps its outer cursor in
`rcx` on main and `r14` in the trials. The frame sizes and stack-reference
counts remain unchanged.
Each prologue also saves eight registers on the stack. The reservation
column gives the separate `sub rsp` allocation.

All callers retain four internal 16-byte alignment directives. The trials
move the first directive from the outer loop header to the whitespace
arrival block. Thus, their instruction reduction also changes hot code
placement. Linked disassembly confirms padding and the dispatch loads.

## Interpretation

The earlier controlled experiment measures release at offset 16 near
407 microseconds and tip at offset 0 near 461 microseconds. Moving the
unchanged tip to offset 32 gives approximately 409 microseconds. The
placement difference exists before PR #135. The earlier table-placement
control shows a smaller variation of approximately 3%.

The current experiment again shows material differences between placements
of identical instructions. Function alignment alone can select a slow
placement. Reversed measurement order alone cannot detect this cause.

The trials remove a dependent load but do not provide a consistent gain
across placements and callers. Their value callers improve at one offset
and have a larger spread across offsets. The PR retains production code.

The processor mechanism remains uncertain. The experiment does not separate
instruction-fetch boundaries, branch prediction, or other frontend effects.
The control also shows residual address and timing variation. Four offsets
do not sample all possible linked addresses or calling contexts.

The isolated experiment covers valid punctuation input. Cold errors abort,
and the copies have no unwind metadata. It does not measure error paths,
callbacks, or retained region helpers. The rejected trials establish no
performance claim for other scan cases.

All 11 existing scan cases pass their benchmark checks. Workspace tests
pass in debug and release. Formatting, Clippy, rustdoc, Rust 1.86 checks,
and the benchmark script tests pass. Production scan generation remains
unchanged for every case.
