# 2. Product vision and scope

## Product contract

raipixel is a lightweight, self-contained, extensible, AI-native pixel-art engine. It MUST support both original artwork and existing-asset manipulation. Its users are developers directing autonomous agents and technical artists automating asset production. The engine is a deterministic executor; an external LLM provides creative reasoning and, when available, visual evaluation.

The creation loop is **author → render → inspect → revise → validate → save/reopen → export**. The import loop enters the same model through PNG and later layered formats. An agent MUST be able to describe an entire small asset with symbolic grids or structured drawing, then change one named part without reconstructing everything else. Original creation is a release gate, not a side effect of a correction API.

Useful original art is realistic for icons, equipment, tiles, effects, geometric props and deliberately authored small characters when instructions are sufficient. Complex anatomy, new poses, occluded detail and unseen directions require actual artistic decisions or supplied artwork. Shapes, transformations and a random seed do not supply those decisions. Visual recognition and quality MUST be evaluated separately from file validity.

## Non-negotiable constraints

The entire application MUST use safe, stable Rust and std. No third-party crates, including dev/build dependencies; no copied/vendored implementation code; no unsafe Rust, application FFI, assembly, build-script dependency, required subprocess, graphics editor, game engine, cloud service or AI framework. It MUST work offline on macOS, Linux and Windows.

Rust's compiler, Cargo, linker, std's platform implementation, filesystems and ordinary OS libraries are permitted facilities. A Rust executable need not be statically linked on every platform. Optional independent consumers used to validate exported files are verification tools, never runtime requirements. Test inputs may be licensed data with provenance; they are not vendored implementation code.

Interoperability between published open formats is a product requirement, not an incidental export feature. Demonstrate import/edit/export conversions with independent consumers and explicit preservation/loss boundaries. Use independently implemented published specifications. Keep graphics, codecs, persistence, validation and agent interface separate. Prefer deterministic integer behavior, explicit targets and ordinary modules. Optimize useful functionality, reliability and maintenance before binary size or line counts. No plugin system, language runtime or dormant framework is justified by extensibility alone.

## Priorities and value

| Priority | Minimum useful capability | Why |
|---|---|---|
| Foundation | Empty canvas and real PNG input; symbolic grids, pixels/lines/rectangles/fills; basic named layer state; palette extraction/exact recolor; alpha key/threshold; inspect/preview/diff; validation; JSON+PNG save/reopen/export | Proves creation and manipulation before advanced systems; PNG is on the critical path |
| Core | Masks and selections, complete basic geometry, exact transforms/composition, cross-project components, palette contracts/quantization, frame clone/edit/order/timing, clips/anchors, multi-target edits, fixed-grid sheets and metadata, controlled seeded variation | First useful toolkit; scenarios A–E all required |
| Advanced | Layered-image interchange, morphology/outlines, ordered dithering, pattern fills, richer inspection, pixel-grid candidate analysis, tiles/maps/collision/seams, additional animation export and indexed PNG output | Valuable extensions with bounded individual contracts and their own workflow gates |
| Deferred | Arbitrary-angle pixel-art resampling, automatic irregular-grid repair, inferred background segmentation, live linked rigs/components, packing/trim, complex topology/terrain solvers, 16-bit/HDR/color management, broad editor-native codecs | Real use cases exist, but each needs demonstrated demand and a reviewed implementation policy |
| Excluded | GUI editor, embedded LLM/model, provider SDK/network service, MCP requirement, external script runtime, dynamic plugins, automatic anatomy/style/pose guarantees | Outside the intended deterministic agent engine |

Exact subfeature classifications are in [F01–F25](05-functional-specification.md); this summary does not promote a whole category to initial scope. Templates are reusable JSON jobs and ordinary component assets. Semantic names are assigned by the author; a flattened PNG does not contain recoverable layers.

The potential advantage is fewer, more reliable agent actions for structured creation and corrections, durable named state, and reproducible output. Drawing, animation and batch export already exist elsewhere; independence alone is not evidence of usefulness. [Comparative tasks](acceptance-scenarios.md) must measure the benefit before any superiority claim.

## Delivery and expansion

The Foundation slice is an independently usable static workflow; it is not completion of the product. The first useful toolkit also MUST pass animation and component gates. Layered interchange is planned Advanced work, not permanently dismissed in favor of flat PNGs. Cross-platform correctness, resource control and format tests start with Foundation and deepen in every slice; they are not postponed to final hardening.

Admit later work only with a representative task, an explicit preservation/algorithm contract, a measured benefit or identified workflow blocker, and acceptable maintenance under the constraints. Investigate individual features rather than rejecting whole editing or animation categories. Feature parity is a long-term reference, not a first-release promise.
