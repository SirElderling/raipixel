# 8. Architectural decision records

Status: proposed design baseline, 2026-10-15. Stable IDs retain decision traceability. “Costly” marks changes likely to require compatibility work. Decisions are judgments, not implementation results.

| ADR | Decision / alternatives and reasons | Consequence |
|---|---|---|
| 001 Identity | raipixel is the sole project/package/executable identity. | Product-focused docs; no alternative names/history. |
| 002 Independence | Safe stable Rust/std, zero third-party code/dependencies, no application FFI/subprocess/network. Preserve ordinary OS/toolchain facilities. | Handwritten format cost is intentional; no optional/dev dependency loophole. |
| 003 Pixels **costly** | One straight RGBA8 raster; RGB palettes are constraints, not an indexed backend. Preserve hidden RGB rather than canonicalize imported samples. | Four bytes/pixel; exact untouched-cel guarantees; explicit compositor base-copy policy. |
| 004 Editable state | Named assets/layers/frames/cels with offsets, pivots/anchors; no inferred semantics or scene/rig graph. | Flat images need supplied selections/parts; useful creation/editing remain first-class. |
| 005 Copies | Immutable Rc image sharing with budgeted detach; explicit target arrays instead of live links. | No accidental propagation; source snapshot semantics for aliasing. |
| 006 Persistence **costly** | **Supersede restricted grammar/RPX with versioned data-only JSON + PNG surfaces.** Replay-only, archive and OpenRaster-only alternatives cannot justify their preservation/complexity trade-offs. | Reuses required parsers/codecs, independently inspectable images, full state in manifest; more files and weak initial compression accepted. |
| 007 Transfer | Copy one asset plus referenced palette/style dependencies; equal reuse, conflicting values fail. | Reuse between independently produced projects without merge framework/live references. |
| 008 Agent interface **costly** | **Supersede line recipes with typed JSON jobs.** No hybrid drawing grammar, loops/params/expressions; compact grids/target arrays supply high leverage. | Slight punctuation cost; one escaping/parser contract, schema discovery and JSON Pointer recovery. Token advantage remains unproven. |
| 009 Publication | Private working state, new staged directory, rename commit; source immutable. | Simple rollback; trusted parent needed; power loss/transport after commit explicitly limited. |
| 010 Geometry/determinism **costly** | Integer algorithms, exact transforms and versioned seeded operations; preserve old coverage definitions. | Reproducible outputs; arbitrary-angle quality remains Deferred. |
| 011 Palettes | Exact extraction and simultaneous maps first; nearest mapping Core; Bayer4 Advanced. Palette updates never recolor implicitly. | Efficient authoring/consistency without perceptual model or silent artwork loss. |
| 012 Masks/effects | Ordinary binary image masks, explicit region/mask writes; effects built from recolor/stamp, morphology later. | No effect graph, hidden selection engine or automatic object inference. |
| 013 Animation **costly** | Core frames/timing/clips/anchors, named component edits, supplied or explicitly authored poses. | First useful toolkit includes animation; no learned pose/view invention promise. |
| 014 Tiles | Advanced authored rectangular sets/maps/labels/collision; existing map interchange profile before delivery. | Not mandatory before first useful toolkit; exact strips opt-in, no terrain solver. |
| 015 PNG import **costly** | **Move real PNG import into Foundation**, full required DEFLATE and documented 8-bit/sub-byte/Adam7 profile. | Largest early correctness cost; prevents creating an isolated advanced editor before importing real art. |
| 016 PNG output | Conforming RGBA8/filter-None/stored DEFLATE for export and native surfaces; indexed output Advanced. | Larger bytes; compressor work only after measurements justify it. |
| 017 Interchange | OpenRaster flat normal-layer subset and APNG export Advanced; editor-native readers individually Deferred. Verify actual specifications, no universal round-trip claim. | ZIP/XML/rational timing add work later; raster interchange never substitutes for full native state. |
| 018 Export **costly** | Explicit untrimmed fixed grids, identity/timing/registration, padding/extrusion and versioned JSON; reusable request files instead of preset entity. | Predictable game placement; no automatic importer for every engine or packing framework. |
| 019 Inspection/validation | Objective reports, bounded pixel/symbol views, diffs and independent consumer checks; visual review separate. | Technical pass cannot certify anatomy/style/motion. |
| 020 Extensibility/KISS | Cohesive modules with private internals and ordinary functions; typed enum/descriptors. **No replacement-backend traits or plugin machinery before an actual need.** | New implementations may later motivate a small interface; current scope stays simple, zero-dependency baseline unchanged. |
| 021 Build/size | Correctness/maintenance first, measure stable build profiles and clean-platform behavior. Retire guessed size/LOC targets. | Ordinary CRT/OS libraries allowed; no universal static-link claim. |
| 022 Resources **costly** | Bound parser/codec/pixel/structural/reference work and payload, not pixels alone. | Local abuse limits without claiming a complete sandbox, RSS cap or universally recoverable OOM. |

Each substantial change needs a workflow or correctness justification, updated related contracts, and meaningful tests before implementation. No dormant generic interface, database/cache, second language, native binary codec, broad format suite or advanced vision subsystem is justified solely by possible future use.
