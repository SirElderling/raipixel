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
