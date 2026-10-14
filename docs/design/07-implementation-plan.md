# 7. Implementation plan

No application implementation is part of this proposal. Implement the normative contracts through small accepted vertical slices. Each milestone is independently usable; unsupported later commands report not-implemented with a supported-feature list, never succeed partially.

## Estimates and accounting

Planning ranges, not measurements. Production Rust LOC excludes tests, documentation, fixtures and generated build output; includes parser/error/help code, handwritten codecs and all selected behavior. Ranges reflect integration/review uncertainty and should be replaced after M1/M4 with actual counts. Fewer lines are desirable only when behavior and maintainability survive.

| Subsystem | Production LOC | Effort contribution / maintenance | Reuse and binary implications |
|---|---:|---|---|
| Domain, native state, asset transfer | 1,700–2,900 | Medium; schema/ownership invariants expensive to change | One asset for sprite/frame/tile; small data-oriented code |
| Geometry, transforms, composition | 1,700–2,700 | Medium; exact coverage corpus | Shared drawing/mask/sheet traversal; integer code, no GPU/runtime |
| Masks, palettes, texture | 1,300–2,300 | Medium; alpha/tie/alias semantics | Recolors/ramps/shadows reuse maps and masks |
| Animation render/previews/export | 1,400–2,200 | Medium; metadata correctness | Same frame composition; fixed grid eliminates packer |
| Tile/map metadata/checks | 700–1,100 | Low/medium; bounded topology | Frames reused, no terrain solver |
| PNG/DEFLATE/RPX/palette codecs | 3,000–4,800 | High; malformed-input and interoperability review | Largest mandatory cost; no compressor/model libraries |
| Recipe, CLI, JSON and help | 1,600–2,700 | Medium/high; contract synchronization | One parser, output-only JSON, no embedded language |
| Validation, limits, transaction integration | 600–1,300 | High correctness burden, distributed helpers also counted above | Shared preflight/rule traversal, no service framework |
| **Total** | **12,000–20,000** | **8–16 experienced-engineer weeks** including tests/review | **Roughly 0.8–4 MiB stripped executable**, low confidence; Windows static CRT may exceed range |

Subsystem effort is not independently additive because tasks share integration work. Expected test Rust is roughly 5,000–10,000 additional LOC plus binary/text fixtures; no external test crate. Eight-to-sixteen weeks describes focused experienced human engineering, not calendar commitment or predicted AI wall time. Allocation ceilings are not runtime memory benchmarks.

## Incremental milestones

| Milestone | Scope / usable deliverable / dependencies | Added and cumulative production LOC | Effort estimate |
|---|---|---:|---|
| M1 Creation-to-export | Basic model/limits; recipe header and declarations; draw pixel/line/rect; simple composition; RPX + restricted snapshot; minimal inspect/validate; stored-block RGBA PNG writer; frame export; help/errors. Empty-project sword create/save/load/render/edit loop works. | +2,800–4,300; 2,800–4,300 | 2–3 weeks |
| M2 Semantic correction | Ellipse/polygon/fill/stamp/transforms, masks/morph/outline, palettes/alpha/dither/style; cel/frame COW edits; bounded native import-asset. Uses M1 stable state. Real layered asset correction works through RPX or native sources; PNG import not yet claimed. | +1,800–3,100; 4,600–7,400 | 1–2 weeks |
| M3 Animation production | Durations/clips/anchors; contact/onion/diff; sheet+JSON/components; presets/padding/extrusion; parameters/flat repeat/texture. Uses M1/M2. Four-frame supplied-pose walk and20 variations export. | +1,900–3,200; 6,500–10,600 | 1–2 weeks |
| M4 Existing-art interoperability | Reviewed zlib/inflate + PNG reader, filters/sub-byte/Adam7, indexed writer, palette files, sheet slicing. Uses M1 model/limits and M3 sheet contract. Ordinary supported externally generated PNGs enter correction loop. | +3,000–4,700; 9,500–15,300 | 2–4 weeks |
| M5 Environment and collection | Rectangular tile/maps/edges/collision/walkable, seam/map previews; full validation/inspection details, shared-style collection scenarios. Uses M2/M3; PNG import from M4 useful but isolated tile algorithms independently buildable. | +1,100–1,900; 10,600–17,200 | 1–2 weeks |
| M6 Release hardening | Full conformance/scenario suite, clean-platform packaging, bounds/clone/codec resource review, output/retry behavior, measured binary/runtime profiles; agent usability test and consumer integration. No new artistic systems. | +1,400–2,800; **12,000–20,000** | 1–3 weeks |

M1 native manifest must already use loader-only surfaces; do not use a throwaway persistence scheme that M2 replaces. M1 declarations need only implement accepted subset, while reserved later keys reject clearly. Codec development can run independently after the image/error/limit contracts exist; M4 need not wait for all animation code. Critical path is model/persistence → semantic operations → animation/export, with PNG import/security verification the largest release risk. M5 topology can proceed alongside codec testing.

## Tests, acceptance, risks and done per milestone

| Milestone | Meaningful tests and acceptance | Main risk / definition of done |
|---|---|---|
| M1 | Complete sword recipe accepted for delivered primitives; known raster/alpha vectors; snapshot preservation; independent decoder/viewer validates emitted PNG; invalid recipe publishes nothing; immutable source retained | Native snapshot/transaction mistakes. Done: executable produces visible32×32 artifact, reload/edit/export works, tests pass on primary host and spec subset documented. |
| M2 | Shape golden masks, concave/even geometry, fill/pattern alias, overlap stamp, COW independent edits, palette ties/Bayer one-color, canonical alpha, resource preflight, native asset transfer including dependency conflicts | Over-shared edits or silent color changes. Done: scenarios B/D semantic subset complete, untouched state byte-equivalent and mask/alpha tests pass. |
| M3 | Timing/playback expansion, clip repeated names, eight supplied directional views, fixed-grid formula, components at scale2, padding/extrusion/non-overlap, bounded parameters/escaping,20 deterministic variants | Metadata identity/registration bugs. Done: scenarios A/F using supplied/native art; consumer reads PNG/JSON timing/pivot without filename heuristics. |
| M4 | Supported PNG matrix; fixed/dynamic/stored deflate, overlap/backreferences/degenerate trees; CRC/Adler/truncation/limits; sub-byte/Adam7 all filters, empty/multiple IDAT; independent fixtures; import/sheet export round trips; unsupported16-bit/APNG precise errors | Handwritten codec safety/correctness. Done: source corpus independently generated and accepted, malformed corpus no panic/over-limit expansion, scenario D existing-PNG path complete. |
| M5 | Tile label adjacency, declared-strip exact rules, checkerboard counterexample, collisions, map order/bounds, seam previews; dozens of assets shared style; false metadata/anchor failures | Mistaking a diagnostic for artistic validity. Done: scenarios C/E fully supported and exact warning/error meanings verified. |
| M6 | All feature contracts F01–F25; examples as integration fixtures; cross-target byte equality; publication/broken-pipe tests; limits at/beyond boundaries; game consumer; representative release performance measured; stable toolchain/no dependencies/unsafe checks | Under-tested portability/resource exhaustion. Done: all three target reports, documented known limits and measured artifact sizes; zero unresolved material review findings. |

## Test facilities and corpus

Use `#[test]`, integration test binaries and `std::process::Command` only in tests to invoke the built raipixel binary. Production engine never spawns a process. Cargo tests build with no dev-dependencies. Deterministic small generated test inputs substitute for a property-testing crate: enumerate geometry/alpha/seed cases and mutate known valid byte sequences with a tiny test-only deterministic generator. Test the independent expectation, not a second copy of the algorithm under test.

Golden images are raw expected RGBA/RPX arrays or checked-in small PNGs with exact pixel expectations. Export byte goldens verify canonical format output; decoded-pixel goldens verify content. Codec corpus must include independently produced fixed/dynamic/Adam7 images and documented provenance/licenses; writer→reader round trips alone cannot establish conformance. Public licensed PNG conformance fixtures or manually constructed RFC cases may be checked in; no external program is required to run tests.

Malformed-input corpus: every truncated prefix of compact files; bad magic/length/type/CRC/Adler; invalid palette index/filter; unsupported depth/APNG; deflate invalid repeat/tree/end/distance; exact-boundary allocations; oversized dimensions or command expansion; metadata/reference clone amplification; paths/traversal/symlinks. Tests assert specific codes and no source/output mutation. Resource tests lower limits to small values rather than forcing a machine OOM. Root permissions/platform differences must not make symlink/I/O tests falsely pass; gate platform-specific fixture creation and document skipped capability.

Cross-platform: run native cargo test on aarch64-apple-darwin, x86_64-unknown-linux-gnu and x86_64-pc-windows-msvc. Compare canonical fixture bytes (PNG, RPX, manifest, JSON) directly across target runs; checksums alone are not proof. Normal compiler toolchain libraries allowed. Optional CI orchestration is not an application dependency; do not introduce third-party actions/frameworks as mandatory runtime or test libraries.

## Implementation handover discipline

For each milestone: identify feature IDs and named acceptance scenarios; write failing behavior tests from the contract; implement the smallest slice; run cargo fmt --check, cargo test, cargo clippy --all-targets -- -D warnings using stable tooling; inspect representative rendered artifacts; record delivered scope and any remaining known unsupported features. Do not broaden tests repeatedly without a new concern. Do not replace PNG import with a shell command/library or add unsafe to improve size.

A milestone cannot be marked done through a compiler pass alone. A separate reviewer checks artifacts/contracts and meaningful failure cases. Before changing a high-cost decision (pixel encoding, IDs/coordinates, persistence, parser semantics, export schema, seed/composition math), update an ADR and tests first. Implementation changes that exceed estimate are explained by added utility/correctness, not squeezed into arbitrary LOC limits.

## Future evolution

Additional codecs implement bounded decode/encode at the existing image boundary. More animation uses explicit new pose/component operations and state only when necessary. Procedural extensions add typed operations/seed contracts; no graph runtime until use proves it necessary. Engine adapters consume current PNG/JSON, with engine-specific files only after a concrete consumer request. Optional interfaces call the library; GUI history/selection state belongs to that consumer. All future format/algorithm additions retain deterministic versioning and dependency constraints unless the user explicitly changes the product constraints.
