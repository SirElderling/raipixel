# 10. Final recommended design

Build **raipixel**, a safe stable-Rust library plus compact CLI for deterministic pixel-asset creation, correction, animation assembly and export. Preserve the zero-crate/no-unsafe/no-FFI/no-network/no-model/no-runtime-program constraints. The product is general-purpose; fantasy RPG assets form the initial acceptance collection.

## Recommendation

Use one RGBA8 image representation throughout. A project owns named palettes/styles, assets and rectangular maps. Each asset owns ordered layers/frames, cels with integer offsets, pivot/anchors, clips and optional tile metadata. Frame clones share immutable Rc image data and detach for explicit edits. A reusable component is an ordinary asset/cel; native asset transfer preserves semantics between separately produced projects.

Use a small line-based typed statement interface shared by individual CLI edits and batch recipes. Explicit keys/targets, scalar parameters and bounded flat repetition provide expressiveness without a programming-language runtime. Machine-readable inspect/validate/error/help output supports agents; PNG images support external visual assessment. No natural-language interpreter is embedded.

Persist current state in immutable self-contained manifest/RPX directories. A restricted manifest references explicit numeric surfaces, preserving sharing and every semantic field. Recipes create/reuse operations; snapshots preserve editable results without depending on changed external sources or algorithms. Publish a new directory only after successful processing; the original remains intact.

## Exact release capability

F01–F25 in the functional spec are the release contract: documents/semantic cels, integer drawing/fill/copy/composition/transforms, masks/morph/outline/shadow, shared palettes/exact ramps/quantize/Bayer/alpha cleanup, seeded texture/scalar templates/bounded batches, frames/clips/timing/pivots/anchors and inspection previews, rectangular tile/maps/authored transitions/metadata, objective checks, bounded PNG import/export and sheet slicing, fixed-grid sheets/padding/extrusion/JSON/component export, native persistence, RGB palette exchange and native asset import.

PNG import includes all legal8-bit/sub-byte grayscale/indexed combinations and Adam7 using a full bounded DEFLATE reader. It rejects16-bit/APNG explicitly. The writer uses simple stored DEFLATE and filter None; RGBA default, exact indexed output optional. This is the largest implementation commitment because existing-art modification is indispensable. It is a clearly documented supported subset, not a promise of complete PNG conformance.

Game export preserves untrimmed source dimensions, frame identities/timing, pivot/anchors, clips, component offsets and tile/collision metadata. Sheet layout is a fixed explicit formula. Pixel edge equality is an optional declared tile contract, not an artistic seamlessness certificate.

## Deliberate exclusions

No GUI, model/client/service, runtime plugin system, scene/rig framework, IK, pose/view inference, automatic semantic segmentation, arbitrary-angle advanced scaling/rotation, atlas packing/trim, full autotile solver, isometric/hex topology, animated GIF/APNG or native editor-format codecs in the baseline. Supplied angled component poses are required when quarter-turns cannot express the intended equipment orientation. Contact/onion/diff PNGs and timings JSON provide external inspection without another animation codec.

## Why this is the smallest practical solution

The essential unit is an editable layered asset, not a flat image export demonstration. Named cels and explicit timing/registration address real correction/game-integration needs. One raster engine serves creation, effects, frames, tiles, masks and sheets. One tokenizer serves recipes and restricted manifests; JSON is output-only. Palette maps serve variants/shading/recolor; mask operations serve outlines/shadows/occlusion. Native transfer reuses the loader rather than adding a merge framework.

PNG and accurate state/metadata remain despite their cost. GUI, model, language runtime, indexed backend, effect graph, packer and general rig are entire subsystems deliberately avoided. This optimizes useful capability per line; it does not claim that a production suite under these constraints can honestly fit in 1,000–3,000 lines.

## Extending it

Add an ordinary algorithm function, typed operation/parser/help entry and meaningful contract tests. Typical additions touch 2–5 production files; a codec 2–3; a validator 1–2. Only new persistent semantics require domain/schema changes. A future GUI consumes the library; engine-specific adapters consume PNG/JSON until a real need justifies new output.

## Estimates and practical limits

Planning estimate: **12,000–20,000 production Rust LOC**, another roughly 5,000–10,000 test LOC plus fixtures, **8–16 focused experienced-engineer weeks** including review/interoperability/portability. Stripped executable roughly **0.8–4 MiB** per target with low confidence; static Windows CRT can exceed it. None are measured results, targets or AI wall-time forecasts. Measure stable s/z/LTO/strip profiles and revise after M1/M4.

The largest trade-offs are handwritten codec cost, larger uncompressed-style PNG files, fixed-grid texture overhead, one explicit copy/edit model instead of live rigs, and reliance on externally supplied artistic poses/views. Memory/work/structure budgets bound local workloads; true OOM, power-loss durability and hostile filesystem races are not universally solved by std-only code.

Expected usefulness: a reproducible workbench that can create icons/equipment and procedural primitives, repair imported assets, preserve targeted edits across sessions, assemble supplied coherent character/action animations, produce rectangular tilesets and enforce collection-wide technical constraints. It makes AI asset production more controllable; it does not replace artistic reasoning or guarantee high-quality artwork by itself.

All six [practical scenarios](acceptance-scenarios.md) are supported within these limits. Implement M1's visible creation/save/edit/export workflow first, then semantic correction, animation, existing-PNG interoperability, tile/collection workflows and release hardening. The [independent review](review/README.md) changed the design before finalization.
