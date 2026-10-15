# Full-codec timing protocol

Run `python tools/png_timings.py --baseline COMMIT --output timings.json` with
Rust 1.99+ and Python 3. Only their standard libraries are required. Native CI
runs this on Linux x86-64, macOS ARM64 and Windows x86-64. Raw results and source
revisions appear in the job log as one JSON record. Timings are informational;
correctness and exact output comparisons fail the job on disagreement.

The runner builds the same current `examples/png_bench.rs` driver against two
separate library revisions with the same compiler, release profile and flags.
It uses baseline/current/current/baseline process order. Each process calibrates
four operations separately to at least 25 ms per batch, then records seven
batches, rotating operation order. The report retains all samples and iteration
counts, pooled medians and min/max ranges; compare within one host/run. Shared
runner scheduling and frequency changes remain noise, not an algorithm gain.
The two current processes and two baseline processes expose temporal drift.

Measurements include fresh output allocation/destruction, checksums, validation,
work accounting and the whole public codec call. File I/O, input generation,
round-trip checks, reporting, compilation and process startup are excluded.
Operations: decode compressed input, decode stored PNG, stored encode, compressed
encode. Synthetic original workloads cover 16/64-pixel icons, a repeating 1024
sheet, a 1024 structured texture and deterministic 256 noise, including hidden
RGB at alpha zero. Compressed decode inputs here use our own fixed-code writer;
this suite does not replace the dynamic/Adam7 corpus tests or independent crate
comparison. Output size is reported. Before timing, both builds export both
writers' files; Python compares their complete SHA256 maps. Every process also
checks decoded pixels against the generated image outside timing.

For any external PNG (including independently compressed dynamic inputs):
`cargo run --release --example png_bench -- INPUT.png 25`. It reports seven
samples per operation as JSON. `--suite 25` runs built-ins; `--export DIRECTORY`
writes untimed suite outputs. Use `tools/compare_png.py` for separately locked
`png` crate comparisons and independent Pillow verification; that optional
runner's dependencies are isolated from the root package. No reference crate
implementation source is inspected or copied.

The paired tool requires a baseline supporting the current public codec API.
It records the resolved commits, working-tree state, Rust version/LLVM/target,
OS/architecture, flags, driver/library SHA256 fingerprints and CI run ID. A nonempty working-tree field identifies a
local experiment; published measurements must name the tested source change.
No performance thresholds are enforced across heterogeneous machines.


## Stage and candidate probes

`cargo run --release --locked --offline --example png_stages -- INPUT.png`
checks and measures filtering, Adler32, DEFLATE encode, IDAT CRC, inflate and
whole-codec operations for trusted noninterlaced RGBA8 inputs. It reuses our
private implementation source without public profiling APIs; its filter screen
is checked against actual inflated writer bytes. Stage times are isolated and
not additive. Native CI runs the same probe on generated sheet/texture files.

`tools/png_match_trial.py`, `tools/png_filter_adler_trial.rs` and
`tools/png_decoder_profile.py` reproduce the rejected/deferred experiments in
temporary source copies or standalone executables. They add no root Cargo
dependencies or application paths. The [decision record](candidates/lookup-and-stage-tests.md)
describes validity checks, instrumentation limits and creator credits.

The optional `tools/compare_png.py` reference harness now also calibrates each
operation to 25 ms and records all seven samples/counts, in addition to exact
output and independent decoding. `tools/png_timings.py` records both benchmark
executable sizes so a speed change can be assessed against its footprint cost.

`tools/png_writer_profile.py INPUT.png ...` times the actual public compressed
writer stages in a temporary instrumented library and alternates it with the
uninstrumented library. Exact bytes are checked first; timers run per block to
limit perturbation. The report retains paired totals and stage samples so its
instrumentation/layout overhead remains visible. It introduces no production
profiling API. `tools/png_token_replay.py INPUT.png ...` captures real tokens
untimed, verifies complete replay byte equality and times the same bit emitter.
Replay has different memory/branch behavior, so subtraction is not an exact
matching-time measurement. Both probes are now part of native CI.

The current paired runner uses ABBA **per image case**, invoking
`--suite MIN_BATCH_MS CASE` on the identical driver. It generates only that
case, so the baseline/current samples are adjacent rather than separated by a
whole suite. Metadata records `process_scope: "case"`; older reports used
whole-suite process order. This narrows temporal drift, but cannot eliminate
shared-runner scheduling/frequency changes. Compare medians only within the
same run/protocol and examine unchanged control operations before interpreting
a small encoder delta.

`tools/png_bottleneck_compare.py --baseline SHA INPUT.png ...` compares actual
old/current private encoder and checksum modules in one executable. It validates
complete DEFLATE bytes and the scalar Adler recurrence first, then calibrates
each of four stage operations to 50 ms and rotates nine batches. Checksums and
encoder allocation are measured separately; these isolated numbers are not an
additive full-codec profile. Source/compiler/input fingerprints and all samples
are recorded. It is now also run natively in CI for the selected bottlenecks.
