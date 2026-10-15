# 10. Recommendation and significant changes

Build a deterministic safe/stable Rust engine that gives original authoring and existing-art editing equal status. One RGBA model, named layered cels/frames, small typed operations, independently implemented PNG/JSON, and explicit immutable snapshots can credibly grow into the product. Useful original creation requires compact grids, palette symbols, composition and reusable parts from the first loop; basic pixels alone would put too much work on the agent.

## Significant decisions

- **Typed JSON jobs replace line recipes.** Grids and explicit target arrays batch substantial work. One JSON parser serves requests, manifests and reports; no expression/repetition language or dual interface.
- **JSON manifest + standard PNG surfaces replaces RPX/custom snapshot syntax.** The manifest retains every editable field and reference. Source art and past drawing algorithms are unnecessary to reopen. PNG is already required in Foundation, so an extra native raster codec provides no demonstrated benefit.
- **Interoperability is tested across formats.** PNG round trips and independent consumers gate the first toolkit; OpenRaster layered conversion is the first planned Advanced interoperability increment, followed by APNG export. Fidelity reports expose preserved/omitted/converted semantics; custom JSON is never described as an external standard.
- **Real PNG import moves into Foundation.** Creation, targeted real-image correction, preview/inspection, validation, save/reopen and independent export verification form the first static slice.
- **Animation remains Core.** The first useful toolkit must demonstrate timing/registration, named component updates, reproducibility and independent game consumption. It is larger than a narrow image-correction utility.
- **Every F01–F25 subfeature is prioritized individually.** Exact palette extraction and symbolic authoring move forward; complex dither/effects, tile/map workflows and layered interchange have separate later gates. Documented editor-native formats do not acquire open-standard status by being documented.
- **Preservation guarantees are explicit.** Hidden RGB survives native state and ordinary single-layer import/export; copy/mask/recolor operations preserve unrelated samples/state. Flattened exports cannot preserve layer semantics; full color management is not claimed.
- **Keep module boundaries simple.** Ordinary functions and model-owned types, no replacement-backend traits/framework until a demonstrated need. Independence constraints remain unchanged.

## Removed complexity and preserved work

Discard the bespoke line grammar, scalar substitution/flat loops, restricted loader execution mode, RPX binary format, extra custom palette syntax, persisted export-preset entity, fixed file-count/LOC expectations and broad six-milestone commitment. Reusable JSON request files provide templates and export settings without those systems.

Preserve explicit IDs/targets; layered cels; COW independence; timing/pivots/anchors/clips; exact integer raster/compositing/seed rules; source-alias snapshots; masks/palette operations; native asset transfer; fixed-grid padding/extrusion; ordered errors; resource limits; immutable publication and post-commit transport handling. Reclassifying a useful capability as Advanced does not invalidate its research or remove it from the product direction.

## Unresolved questions and evidence

The highest uncertainties are handwritten codec correctness, agent ability to author recognizable art efficiently, real token/correction cost, and adoption value against established workflows. No implementation or comparative benchmark has run. [Acceptance A–E and the comparative protocol](acceptance-scenarios.md) must resolve those questions; prose and schemas cannot.

Stored-block PNG size and multi-file snapshots may need optimization after measurement. OpenRaster subset completeness and external consumer behavior need fixture proof before delivery. Additional native/map profiles, arbitrary-angle transforms, more advanced correction and color management require concrete tasks and preservation policies. No unmeasured development duration or executable-size range is retained as a promise.

Proceed through [capability slices](07-implementation-plan.md), validate actual rendered artifacts, and resolve material findings before claiming support. The [adversarial review](review/README.md) records the four separate perspective checks and their design resolutions; it is not a claim of external certification or executed application tests.
