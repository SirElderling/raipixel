# S1 implementation progress

This increment starts S1 from the [design roadmap](design/07-implementation-plan.md).
It implements the raster prerequisites, not the full Foundation workflow.
The design documents and command schema describe the intended product;
only the library functions listed below are available today. The API is unstable.

## Delivered

- One edition-2024 Cargo library, minimum Rust 1.99, with unsafe code forbidden.
- No dependencies, dev-dependencies, build-dependencies, or build script.
- Positive-size, contiguous row-major straight RGBA8 images. Transparent RGB is preserved.
- Half-open filled rectangles, copy and source-over stamps, explicit clipping,
  and same-image stamps that read a pre-edit snapshot. Calls return discarded pixel counts.
- Integer alpha/opacity rounding as specified in the architecture.
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
and local allocation checks. Aggregate live project memory, structural model,
shared cels, job rollback and file publication remain for their implementations. The library performs no file or process I/O.
There is no CLI, typed job/schema validation, layered frame renderer or
persistence yet.

## Develop

With stable Rust installed:

```sh
cargo fmt --all -- --check
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
```

`tests/raster.rs` contains 22 integration tests using hand-authored sample
expectations: transparent RGB, row order, selection preservation, copy alignment,
overlap, alpha rounding, bounds and the specified limits. One small exhaustive
alpha/opacity check covers the byte-valued compositing domain. `tests/json.rs`
adds 19 tests for the specified JSON syntax, Unicode, limits, exact integer
ranges, emission and the real proposed command schema. Eight PNG tests, two DEFLATE unit tests and three checksum tests add the independently generated codec corpus. All 54 tests
need only the Rust toolchain and run offline.

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
(currently 1.99); older compiler compatibility is not maintained.
Toolchain installation uses the network; Cargo build/test commands run offline.
The workflow runs on pull requests and pushes to main/feat branches.

Next, improve the measured codec throughput, then implement typed job validation
and the creation/import/edit/inspect/save/reopen/export workflow.
No extra backend abstraction or package split is needed for those additions.
