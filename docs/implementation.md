# Initial implementation

This increment starts S1 from the [design roadmap](design/07-implementation-plan.md).
It implements the raster prerequisites, not the full Foundation workflow.
The design documents and command schema describe the intended product;
only the library functions listed below are available today. The API is unstable.

## Delivered

- One edition-2024 Cargo library, minimum Rust 1.85, with unsafe code forbidden.
- No dependencies, dev-dependencies, build-dependencies, or build script.
- Positive-size, contiguous row-major straight RGBA8 images. Transparent RGB is preserved.
- Half-open filled rectangles, copy and source-over stamps, explicit clipping,
  and same-image stamps that read a pre-edit snapshot. Calls return discarded pixel counts.
- Integer alpha/opacity rounding as specified in the architecture.
- Typed errors with interface-compatible category codes; no structured CLI errors yet.
- Per-image dimension/pixel/byte ceilings, fallible pixel-buffer reservations,
  and a cumulative caller-owned pixel-work budget.

The default per-image ceilings use the design values (4096 per side,
16,777,216 pixels, 512 MiB bytes); callers can lower them. Dimensions limit
an actual RGBA allocation to 64 MiB. Work defaults to 1,000,000,000 units and
can be lowered, including to zero. Creation/import/copy charges one unit per
pixel; fills and stamps charge the declared area before clipping; same-image
stamps additionally charge the source snapshot. Preflight errors do not modify
destination pixels. Failed work charges leave the budget unchanged; allocation
failure after a successful charge may consume work.

These are local raster safeguards. Aggregate live memory, parser/codec work,
project structure, shared cels, job rollback and file publication remain for the
corresponding implementations. The library performs no file or process I/O.
There is no CLI, PNG/JSON support, layered frame renderer or persistence yet.

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
alpha/opacity check covers the byte-valued compositing domain. Tests need only
the Rust toolchain and run offline.

TDD history starts with tests against an empty library: `cargo test --offline`
failed with unresolved raster API imports. The following implementation made
all 22 tests pass; formatting and Clippy then passed. The red commit is retained
as development evidence, not a passing revision to use on its own.

GitHub CI runs native stable tests, formatting and Clippy on Linux x86_64,
macOS ARM64 and Windows x86_64 MSVC. A separate Linux test checks Rust 1.85.
Toolchain installation uses the network; Cargo build/test commands run offline.
The workflow runs on pull requests and pushes to main/feat branches.

Next, implement the bounded JSON parser and required PNG codec with their own
tests, then connect creation/import/edit/inspect/save/reopen/export. No extra
backend abstraction or package split is needed for those additions.
