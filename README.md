# raipixel

**raipixel is a lightweight, extensible pixel-art toolkit designed for AI agents and game-development workflows.** It aims to make creating, editing, animating, validating, and exporting 2D pixel-art assets reproducible through a local command-line interface and reusable Rust library.

## Vision

Make pixel-art production an **editable, repeatable process**, not a series of disconnected image generations. An agent should be able to build an asset, inspect the result, change one part without disturbing the rest, assemble animations, verify technical constraints, and export files ready for use in a game.

raipixel provides deterministic graphics operations and structured asset data. Artistic direction and visual judgment remain with the agent or artist; the toolkit does not generate artwork using an embedded AI model.

## Goals

- **Agent-first:** Provide discoverable commands, structured results, reusable recipes, and clear validation errors for automated workflows.
- **Editable by design:** Preserve named assets, layers, frames, and components so targeted revisions do not require rebuilding entire images.
- **Consistent assets:** Reuse palettes, dimensions, alignment, and export settings across characters, equipment, environments, and effects.
- **Practical animation:** Assemble supplied poses and parts into timed animations while maintaining pivots, anchors, and frame registration.
- **Game-ready output:** Export sprites, sprite sheets, tilesets, and machine-readable metadata with predictable layout and behavior.
- **Small and extensible:** Favor a compact, maintainable Rust core with straightforward ways to add operations rather than a large framework.

## Planned features

| Area | Capabilities |
|---|---|
| Drawing and editing | Pixel shapes, lines, fills, copy/paste, integer scaling, mirroring, rotation, masks, and compositing |
| Layered assets | Named layers and cels, frames, reusable components, targeted edits, and persisted editable projects |
| Palette and effects | Shared palettes, recoloring, quantization, dithering, outlines, shadows, transparency cleanup, and seeded variations |
| Animation | Frame timing, clips, directional sequences, anchors, contact-sheet previews, and sprite-sheet export |
| Tiles and environments | Rectangular tilesets, authored transitions, tile maps, edge checks, and collision metadata |
| Automation and quality | CLI commands, batch recipes, image inspection, diffs, validation, deterministic processing, and resource limits |
| Import and export | PNG import/export, native project snapshots, and JSON metadata for game integration |

## Design principles

- **Headless and offline:** No graphical interface, cloud connection, or built-in generative model is required.
- **Predictable:** Operations should produce reproducible output, preserve explicit asset structure, and report failures rather than silently altering data.
- **Portable:** Target macOS, Linux, and Windows.
- **Minimal dependencies:** Use safe, stable Rust and its standard library without third-party runtime or build dependencies.
- **Honest scope:** Technical checks can verify pixels and metadata, but cannot guarantee artistic quality or invent missing poses.

## Project status

**Design and specification stage.** The features above describe intended functionality; the application has not yet been implemented.

The [design documentation](docs/design/README.md) contains the detailed [product scope](docs/design/02-vision-scope.md), [architecture](docs/design/04-architecture.md), [functional requirements](docs/design/05-functional-specification.md), [agent interface](docs/design/06-agent-interface.md), and [development plan](docs/design/07-implementation-plan.md).
