# raipixel

**A self-contained pixel-art creation and manipulation engine designed for AI agents, written in safe, stable Rust.**

raipixel aims to let an agent create artwork from an empty canvas, import and correct existing assets, reuse named parts, assemble animations, inspect results, and export game-ready pixels and metadata. Original creation and existing-asset editing have equal importance.

The external agent supplies artistic decisions and structured instructions. raipixel executes deterministic graphics operations locally, without an embedded model. Any LLM that can produce the documented JSON commands can use it through a host with filesystem and process access.

## Planned capabilities

- **Create:** Compact symbolic pixel grids, palette-aware drawing, shapes, fills, symmetry, repetition, and composition.
- **Edit and reuse:** Named layers and components, masks, targeted corrections, palette extraction/recoloring, transparency cleanup, and exact transforms.
- **Animate:** Editable frames, timing, clips, pivots, anchors, component updates, and diagnostic previews.
- **Produce assets:** Reproducible variations, validation, PNG frames and sprite sheets with explicit game metadata; layered interchange and tileset workflows follow later.
- **Inspect and correct:** Structured reports, pixel-region views, histograms, differences, and actionable errors.

## Principles

Safe, stable Rust and its standard library only: no third-party crates, copied implementation code, unsafe Rust, FFI, assembly, or required external executables. Offline operation on macOS, Linux, and Windows. Ordinary OS and Rust toolchain facilities remain necessary.

Interoperability between open formats is central: use published specifications, independently implement their required behavior, and test conversions with independent consumers. Preserve full editable state in versioned JSON manifests referencing standard PNG surfaces. Favor cohesive modules and useful high-level operations over frameworks or arbitrary code-size targets.

## Status

**Initial library implementation.** Bounded RGBA images, rectangular fills, exact copies (including overlapping copies), integer source-over compositing, and bounded JSON parsing/emission with exact integer conversion are implemented with std-only tests. There is no CLI, typed job interface, PNG codec, project persistence, or animation yet. See [implementation status and development commands](docs/implementation.md).

The Foundation milestone remains incomplete: it must complete both creation and PNG import/edit/save/reopen/export. The first useful toolkit must also demonstrate animation and component reuse. Artistic quality is evaluated separately from technical correctness; missing poses, anatomy, or unseen directions require sufficient authored instructions or source artwork.

Start with the [design guide](docs/design/README.md), [scope and priorities](docs/design/02-vision-scope.md), [agent interface](docs/design/06-agent-interface.md), and [capability roadmap](docs/design/07-implementation-plan.md).
