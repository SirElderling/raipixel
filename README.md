# raipixel

A planned, headless, extensible pixel art suite for AI agents, written entirely in safe, stable Rust with the standard library only.

**Status: specification; no application implementation.** The working brief used the name *PixForge*. The repository, executable, package and product name are **raipixel**.

The agent supplies artistic decisions and source artwork. raipixel supplies deterministic drawing, semantic edits, animation assembly, palette correction, validation and game asset export. It does not contain a generative model.

Start with the [design index](docs/design/README.md), then the [recommended design](docs/design/10-recommended-design.md). Implementers must read the normative domain, functional and interface contracts before starting a milestone.

## Constraints

Stable Rust; no external crates, including dev/build dependencies; no unsafe code, handwritten assembly, FFI, embedded runtime, mandatory executable dependencies, cloud service or network requirement. Targets: macOS ARM64, Linux x86-64 and Windows x86-64. No GUI in the selected scope.

## Documentation

The proposal includes competitive research, evidence grading, product scope, domain model, architecture, functional contracts, exact agent interface, ten workflows, incremental plan, decision records, risk assessment and independent review dispositions. Estimates are planning ranges, not measured code or performance results.
