# S1 implementation progress

This increment starts S1 from the [design roadmap](design/07-implementation-plan.md).
It implements the raster prerequisites, not the full Foundation workflow.
The design documents and command schema describe the intended product;
only the library functions listed below are available today. The API is unstable.

## Delivered

- One edition-2024 Cargo library, minimum Rust 1.99, with unsafe code forbidden.
- No dependencies, dev-dependencies, build-dependencies, or build script.
- Positive-size, contiguous row-major straight RGBA8 images. Transparent RGB is preserved.
- Half-open rectangles, exact bounded Bresenham lines, lossless ASCII symbol grids, four-connected exact-RGBA flood fill, copy and source-over stamps, explicit clipping,
  and same-image stamps that read a pre-edit snapshot. Calls return discarded pixel counts.
- Integer alpha/opacity rounding as specified in the architecture.
- Exact, simultaneous RGB/RGBA recolor using bounded fixed-size lookup, alpha
  key and threshold operations that preserve all underlying RGB samples.
- Exact bounded RGBA histogram, alpha statistics, visibility bounding box and
  optional region pixel inspection (raw region <=4096 pixels).
- Typed errors with interface-compatible category codes; no structured CLI errors yet.
- Per-image dimension/pixel/byte ceilings, fallible pixel-buffer reservations,
  and a cumulative caller-owned pixel-work budget.
- Bounded UTF-8 JSON parsing and emission; complete number syntax, escapes and
  surrogate pairs. Duplicate decoded keys, BOM and unpaired surrogates are rejected.
- Static PNG import/export, including the required DEFLATE, filter and Adam7
  profile; see [codec coverage, architecture and measurements](png.md).
- Exact signed/unsigned 64-bit integer conversion, including integral decimal
  and exponent forms, without floating-point rounding.

The `json` module follows [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259)
with the stricter application profile in the design. It stores validated number
text unchanged; numeric equality/conversion is distinct from text equality.
Its generic emitter sorts object keys, keeps array order, escapes controls and
ends with a newline. This is not the future typed manifest/result writer with
schema-defined field order, or a claim of canonical JSON conformance.

JSON defaults are 8 MiB input/output, 64 container levels, 64 KiB per decoded
string/key or raw number token, and 100,000 values including containers.
Callers can only lower these ceilings. Parsing charges input bytes plus value
visits; emission charges actual output bytes plus value visits. Numeric
conversion traverses only the already bounded token and never expands an
exponent into an allocation. JSON errors expose a byte offset and category
code; typed operation fields and JSON Pointer diagnostics are not implemented.
String/vector reservations are fallible; standard map allocation can abort on OOM.

The default per-image ceilings use the design values (4096 per side,
16,777,216 pixels, 512 MiB bytes); callers can lower them. Dimensions limit
an actual RGBA allocation to 64 MiB. Work defaults to 1,000,000,000 units and
can be lowered, including to zero. Creation/import/copy charges one unit per
pixel; fills and stamps charge the declared area before clipping; same-image
stamps additionally charge the source snapshot. Preflight errors do not modify
destination pixels. Failed work charges leave the budget unchanged; allocation
failure after a successful charge may consume work.

These are local raster safeguards. JSON and PNG have their own charged work
and local allocation checks. The library performs no file or process I/O.
A separate minimal CLI currently supports one asset, one layer, one frame and
one editable cel: create or import static PNG, draw pixels/lines/rectangles and
symbol grids, flood-fill, exact recolor, alpha cleanup, and static PNG export.
Ordered named RGB palettes persist in JSON/PNG snapshots, resolve
`@palette/entry` in drawing commands, and may be replaced without silently
recoloring existing pixels. Reopen, edit, inspect and export are supported.
Read-only inspection accepts bounded targeted pixel queries, deterministic
histograms and project structure summaries. Unsupported fields and snapshot
semantics are rejected instead of discarded. A read-only single-frame
compositor for multiple supplied layers exists in the library, but the
CLI cannot yet manage layered projects or animations.

CLI output is staged in a sibling directory and renamed on success, and dry-run
creates no output. This is not a certified cross-process no-clobber protocol,
and the CLI still lacks complete typed diagnostics, documented exit-code
categories, a standalone validation command and the full S1 project model.
Aggregate live project memory and process-wide RSS remain unverified.
Color-mapping kernel variants are compared in a native benchmark-only
workflow; timing results must be recorded before changing the production
algorithm. No performance superiority is assumed from algorithm selection.

## Develop

With stable Rust installed:

```sh
cargo fmt --all -- --check
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
```

`tests/raster.rs` now contains 32 integration tests using hand-authored sample
expectations: transparent RGB, row order, selection preservation, copy alignment,
overlap, alpha rounding, bounds, exhaustive small-coordinate line coverage,
ASCII grid validation, flood-fill connectivity and the specified limits. One
small exhaustive alpha/opacity check covers the byte-valued compositing domain.
`tests/raster_png_workflow.rs` independently asserts authored pixels for a
border, exact hidden-RGB fill, line and grid correction, then encodes and
reopens the image through stored, compact and indexed PNG writers. It also
checks deterministic bytes and transactional edit rejection. This demonstrates
a static in-memory workflow, **not** a typed CLI or editable-project snapshot. `tests/json.rs`
adds 19 tests for the specified JSON syntax, Unicode, limits, exact integer
ranges, emission and the real proposed command schema. Ten PNG integration tests, two PNG filter unit tests, ten DEFLATE unit tests
and five checksum tests add the
independently generated codec corpus and exhaustive encoder-code checks. The
original 68-test checkpoint is historical; additional raster, optional-quality
and cross-format tests have since been added. Core tests need only the Rust
toolchain and run offline.

TDD history starts with tests against an empty library: `cargo test --offline`
failed with unresolved raster API imports. The following implementation made
all 22 tests pass; formatting and Clippy then passed. The red commit is retained
as development evidence, not a passing revision to use on its own.

The JSON parser tests likewise preceded its implementation. The emission and
numeric-conversion checkpoint ran with 12 tests passing and seven failing;
the following implementation made all 19 JSON tests pass. These red/green
commits are retained and were pushed as work progressed.

An independent development check passed the command schema and all 15 fenced
JSON examples through this parser/emitter, then compared their meanings with
Python's standard JSON parser using exact decimal numbers: 16 documents passed.
Python is a verification facility, not an application or Rust test dependency.

GitHub CI runs native stable tests, formatting and Clippy on Linux x86_64,
macOS ARM64 and Windows x86_64 MSVC. CI follows the latest stable Rust
(currently 1.99.1); older compiler compatibility is not maintained.
A std-only checksum probe also compares previous/current arithmetic in each
native job; checksum equality is checked and timings are informational.
The [full-codec timing runner](../benchmarks/png/timing.md) additionally builds
the same driver against baseline/current libraries and measures both decoders
and writers in ABBA process order. Exact PNG SHA256 equality gates these runs;
raw timings remain informational. Actual decoder-stage profiles separate chunk
validation, inflate and reconstruction, with nested CRC/Adler timers and paired
plain builds to expose instrumentation overhead. The CRC size-policy decision
and all mixed results are recorded in the [decoder report](../benchmarks/png/candidates/decoder-crc.md). Native samples and independent comparison
results are recorded in the PNG benchmark directory.
The [latest Adler decision](../benchmarks/png/candidates/two-stream-adler.md)
records independently reduced packed streams, native complete-codec gains,
unchanged sample/flag/work/output artifacts, small-case regressions and the
remaining crate gap. All 68 core tests pass natively. Rejected row-copy fusion,
dot-product and smaller prior-sum variants remain reproducible; they do not
change production behavior.
Toolchain installation uses the network; Cargo build/test commands run offline.
The workflow runs on pull requests and pushes to main/feat branches.

The optional, feature-gated indexed Balanced PNG encoder has been measured
against the public `png 0.18.1` reference on Linux x86-64, macOS ARM64 and
Windows MSVC. See the separate integrated native CI, including complete input
and output checks. Its conservative capacity planning is **not** a measured
peak-RSS ceiling. Do not promote that opt-in to a default or claim universal
compression parity from the current finite fixture set.

Next, extend the now-working single-cel command workflow to an independently
reopenable multi-layer model with complete typed field validation and a
standalone validation command. Use the existing frame renderer for flattening.
Harden deterministic publication, accounting and path/no-clobber tests
before claiming the whole S1 Foundation milestone. Avoid further codec
micro-optimizations until this complete workflow is available.
No extra backend abstraction or package split is needed for those additions.
