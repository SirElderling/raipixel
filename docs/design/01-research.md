# 1. Capability research and standards assessment

Assessment date: 2026-10-15. This is a scoped documentation review, not an executed benchmark or a market-demand survey. Product-neutral findings below were checked against current primary manuals, automation documentation, generation API descriptions and firsthand public workflow reports. Product identities and their URLs are deliberately absent from project documentation; formal specifications remain linked. Consequently the category findings are not an independently reproducible product comparison from this repository alone and MUST NOT support marketing superiority claims.

## Verified capabilities, anecdotes and judgments

| Evidence type | Finding | Product implication |
|---|---|---|
| Verified primary manuals | Established sprite workflows expose layers/cels, frame timing/tags, palette controls, onion previews, sheets and sometimes tilemaps. | These are useful parity references. Explicit model state and reliable animation are essential; a generic raster utility alone is insufficient. |
| Verified automation documentation | Existing tools already expose headless/batch export, metadata, layer selection and, in some cases, scripting APIs. | “Has a CLI” is not differentiation. Compare actual agent steps and failure recovery. |
| Verified procedural command documentation | Image operators, composition and ordered processing already support repeatable raster production. | Reuse simple operation composition. A new node graph or language adds no demonstrated advantage. |
| Verified published generation interface | External products advertise reference-guided artwork, directions, animation and tilesets through model-backed interfaces. | Do not equate deterministic drawing with learned synthesis. Import their resulting images without embedding their services or promising their artistic capability. Published capability is not independent quality evidence. |
| Firsthand user anecdotes | Concrete historical reports concern ambiguous frame/layer identity in multi-file sheets and needing exported pivots/hitboxes/metadata. | Qualifies explicit asset/frame IDs and registration/consumer tests. A historical report does not establish a current defect or prevalence. |
| Architectural judgment | Compact symbolic grids, batch target lists, named components, strict local writes and machine-readable recovery may reduce agent friction. | Hypothesis to test, not an established advantage. |

Useful prior qualitative task themes are retained: reproducible export settings; palette ownership/near-duplicate colors; partial-alpha preservation; registration across linked parts; sheet identity and padding/extrusion; codec timing; batched foliage/effects; component replacement with occlusion; consistent body/palette reuse; supplied-grid reduction; explicit key-color cleanup; authored tile transitions. Reports without retrievable provenance are hypotheses, not verified evidence. Do not retain unsupported strength scores, consensus claims or arbitrary demand rankings.

The resulting opportunity has **two equal halves**: high-level original authoring from explicit instructions and precise manipulation of existing editable/imported art. A repair-only scope misses the product; a promise to invent coherent character animation from primitives overstates deterministic graphics. Real usability and visual-quality studies are still missing.

## Standards and format facts

| Specification | Verified semantics / implication |
|---|---|
| [PNG Third Edition](https://www.w3.org/TR/png-3/) | W3C Recommendation dated 2025-06-24. Defines static raster storage and APNG animation; legal samples extend to 16 bits, with filters/interlace/color metadata. A supported 8-bit profile must not be called complete decoder conformance. No editable named layer/anchor model. |
| [JSON RFC 8259](https://www.rfc-editor.org/rfc/rfc8259) | Standard data syntax, not an application command/asset model. One independently implemented parser can serve jobs, manifests and structured output. Explicit application validation remains necessary. |
| [zlib RFC 1950](https://www.rfc-editor.org/rfc/rfc1950), [DEFLATE RFC 1951](https://www.rfc-editor.org/rfc/rfc1951) | PNG compatibility requires wrapper checksums and stored/fixed/dynamic decompression, not just the engine's own stored-block writer. No compression dependency is necessary if correctly implemented independently. |
| [OpenRaster file layout](https://www.openraster.org/baseline/file-layout-spec.html), [layer stack](https://www.openraster.org/baseline/layer-stack-spec.html) | Open layered interchange: ZIP, stack.xml and referenced images. Mimetype first/stored, mandatory merged image and thumbnail. Stack order is uppermost first. Supports layer offsets, visibility, opacity, groups/blends; baseline is not an animation/clip/registration specification. |
| [OpenRaster proposals](https://www.openraster.org/proposals/proposals.html) | Animation and palette proposals are distinct from established baseline guarantees. Do not treat a proposal as existing interoperability. |
| [ZIP APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT), [XML 1.0](https://www.w3.org/TR/xml/) | Additional parser/container obligations for layered interchange. Reuse DEFLATE but still budget ZIP/XML work, paths, entities and unsupported features. |
| [JSON Schema Draft 2020-12](https://json-schema.org/draft/2020-12/json-schema-core) | Published schema specification for discovery and structural validation. Emitting a schema does not require implementing a general-purpose schema engine. |
| [Rust module rules](https://doc.rust-lang.org/reference/items/modules.html) | Ordinary module boundaries hide internals without a runtime replacement framework. Package splitting/traits are not prerequisites for cohesive design. |

Documented editor-native and tile-map formats were also inspected. They are project/vendor-specific specifications rather than neutral standards. Some editor formats contain layers, linked cels, palettes, timing/tags, slices/pivots and tile data, but those features cannot be assumed for every format or preserved by a generic importer. Documented map schemas include global IDs/flags, layers and compressed/infinite variants; “JSON” alone does not imply compatibility. [Architecture](04-architecture.md) makes concrete adoption/profile decisions without exposing external product identities.

## Interface and persistence judgments

Choose typed JSON: its standard syntax/schema/tooling offsets extra punctuation, while symbolic grids and batched targets reduce the dominant artwork payload. No measured token comparison currently proves it cheaper than lines. Retaining both grammars would duplicate error/default/escaping contracts without adding graphics capability.

Choose JSON manifest + PNG surfaces: required command JSON and Foundation PNG already pay the parser/codec cost. Raw RPX could decode faster and avoid PNG overhead but introduces another private format and cannot be independently inspected by standard image consumers. Standard PNG surfaces improve inspection/interchange; the versioned manifest preserves the missing editable semantics. File count/stored-block sizes and decode performance remain measurements, not reasons for a speculative database/archive/cache.

OpenRaster is valuable layered interchange but insufficient as the sole native project because baseline does not preserve the full animation/anchor/component model. APNG carries rendered frames with fractional delays and disposal/blend semantics, not editable cels. Native state remains necessary; avoid private chunks pretending those raster formats preserve the entire project.

## Validation priorities

[Acceptance tasks](acceptance-scenarios.md) cover original art, real import correction, named component animation edits, variants, game export and agent costs. Record baseline versions, exact instructions, commands, fixtures and failures when tests run. Current evidence supports investigating these workflows, not promising commercial value, quality parity, speedups, adoption or a development schedule.


## Research attribution requirements

Any algorithm, technique, design or optimization adopted fully or partially
from a paper MUST acknowledge that paper's creators in project documentation.
This requirement applies to adaptations and independently written
implementations as well as direct use of a described algorithm.

For each adopted contribution, documentation MUST record:

- The authors, paper title, publication year/venue when available, and a stable
  public link or DOI; include a freely accessible full-text link when available.
- The particular idea used, where it appears in the design/code, and the changes
  or simplifications made for this project. A nearby code comment SHOULD link
  to that documentation when it helps maintainers identify the contribution.
- Which correctness/performance claims come from the paper and which have been
  independently verified here. A paper's reported speedup MUST NOT be presented
  as a raipixel result without our own measurements.

Papers reviewed but not adopted MUST be labeled separately; citing a paper
MUST NOT imply that its full algorithm was implemented or that its authors
endorse this project. When further work changes the adopted technique, update
the attribution with the implementation. Acknowledgments do not relax the
safe-Rust, std-only or no-copied-implementation-code constraints.

These requirements cover algorithm literature; the product-neutral capability
assessment above is a separate scope. Research authors and paper links MUST
remain identifiable even when product comparisons are summarized neutrally.
