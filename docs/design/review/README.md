# Independent adversarial review and resolution

Four independent reviewer agents examined the draft contracts before finalization. Traditional-editor and AI-product research were also investigated independently. Reviewers supplied findings without changing files or implementing the application. The author reconciled the specification and requested follow-up interface/graphics checks on concrete examples. This is an architectural review, not evidence that unimplemented software passed tests.

## Material findings and design changes

| Perspective / challenge | Final change | Disposition and verification |
|---|---|---|
| Rust: pixel-only limits permit billions of cheap shared references | Global structural/reference/string caps; clone/import preflight; parsed/expanded storage budget; parser/codec/metadata work charged | Resolved in domain/architecture/interface. Future resource fixtures required. |
| Rust + agent: native snapshot cannot reference RPX/shared surfaces in grammar | Loader-only surface declaration and cel surface field; exact canonical order/restricted commands | Resolved, interface follow-up confirms. |
| Rust: off-canvas geometry can overflow or loop excessively | Signed i32 coordinates, bounded extents, wide arithmetic and checked conservative work preflight | Resolved; regression fixtures in M2/M6. |
| Rust: broken stdout after rename defeats any-error rollback promise | Rename explicit commit boundary; committed result retained; publication/path report and retry guidance | Resolved; M6 transport test required. |
| Rust: standalone Windows linkage unresolved | Select MSVC target with stable static CRT flag; clean packaging validation | Concrete decision made; measurement still pending implementation. |
| Graphics: exact edge equality rejects valid periodic checkerboards | Structural/label checks separated from optional strip-equality contract; diagnostic seam mosaics; representative-group algorithm | Resolved; checkerboard counterexample added to acceptance. |
| Graphics + agent: undefined tolerance/pair/expanded playback previews | Single-tile seam preview, map preset, exact comparison only; unique frame contact/sheets, clips remain metadata | Resolved without adding operations. |
| Graphics: PNG uniqueness wording and pass/header details ambiguous | Explicit repeated/empty IDAT, zlib header validation, filter stride, per-pass previous-row reset, raw tRNS comparison | Resolved; supported/malformed corpus gates retained. |
| Graphics: alpha key/pattern alias/component scale unclear | match=rgb/rgba, immutable pattern reads, one-color dither, straight alpha/color policy, scaled+logical component metadata | Resolved; follow-up graphics check confirms coherent contracts. |
| Agent: references cannot be cleared/deleted for ordinary correction | Explicit asset clear fields; cel/clip/anchor/metadata deletion | Resolved without a generic mutation language. |
| Agent: interim artifacts not expressible | All in-job presets observe final state; intermediate inspection uses separate snapshot jobs | Resolved. |
| Agent: JSON result contracts/count meanings incomplete | Typed success result examples, histogram/bbox/count/artifact/registration schema meanings | Resolved in interface appendix; implementation fixtures required. |
| Product: independent native component reuse missing | Minimal import-asset of one asset + dependent palettes/styles; conflict failure, no merge framework | Resolved; added F25 and native reuse scenario. |
| Product: extrusion both selected and deferred | Retain small bounded extrusion; defer packing/trim only | Scope made consistent; direct U5 request supports selected choice. |
| Product: arbitrary rotation demand understated | Acknowledge held-tool angles in U13; require supplied angled poses in v1 | Resolved; demand real, codec/quality/code trade-off still favors deferral. |
| Rust: lower LOC estimate too optimistic | Raise planning band to 12k–20k, focused effort 8–16 weeks; tests estimated separately | Resolved; no fabricated measurements. |

## Independent follow-up on workflows

Agent reviewer checked all ten recipes against operation keys and owner sequencing; graphics reviewer independently calculated sword coverage, seed-driven texture coordinates, terrain strips and sheet layout. Both confirmed the proposed walking clones preserve body/timing/anchors and the 136×34 example layout is correct. The graphics review found texture x=17 could recolor a blade outline at seed 0; the example's interior region was narrowed to width2. The agent review corrected a 4×3 panel description to12 pixels.

No reviewer recommended a richer language, node graph, plugin framework or another raster backend. Independent product checks supported conservative historical/anecdotal evidence labels and the U3/U8/U13 observations. Remaining uncertainties concern empirical implementation cost, codec correctness, agent usability and actual artistic input quality; they are future milestone gates, not silently claimed to be resolved through prose.

## Release review gates

Every selected feature has a normative behavior/acceptance contract. Implementers still need independent codec fixtures, actual three-platform tests, actual images/consumer checks and measured sizes. No application code, build or application test result is present in this documentation PR. The current review resolves design disagreements; M6 cannot be signed off solely from this report.
