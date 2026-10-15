# 2. Product vision and scope

## Definition and users

raipixel is a local deterministic asset workbench for agents that can write UTF-8 files, invoke a CLI, inspect PNG images and consume JSON. Its reusable Rust core creates and modifies raster assets, maintains named editable parts, assembles animation and tiles, checks objective constraints, and exports game-facing pixels and metadata.

Primary users are developers directing AI coding agents through game asset production. Secondary users are technical artists running reproducible batches. There is no assumption that the agent can draw convincing anatomy or recognize every defect. Humans retain artistic approval.

The first validation collection is a fantasy RPG: characters/NPCs/creatures, equipment/icons, terrain/buildings/dungeons, effects and UI. Asset category is metadata, not an engine type. The same primitives support science-fiction props or abstract interfaces.

## Product promise

Generate → render → inspect → diagnose → modify → validate → export must work without reconstructing untouched parts. A named armor cel can change while body pixels, frame order and timing remain unchanged. Supplied directional artwork can share a palette and alignment contract. A deterministic recipe can produce twenty controlled variants and preserve their editable snapshots.

Differentiation is a compact agent contract and durable editable state under unusually strict runtime constraints. Existing tools already support forms of headless editing or image generation; raipixel does not claim those capabilities are novel. It focuses on local, deterministic correction and assembly of source artwork. Neither automatic artistic quality nor automatic unseen-view synthesis is promised.

## Principles and success measures

Usefulness takes precedence over line count. Reuse raster/mask/composition operations before inventing a subsystem. Prefer ordinary Rust modules and typed commands. Persist state explicitly. Reject silent clipping, silent palette correction and metadata loss. Maintain deterministic bytes for supported operations across the three targets under the same engine semantics version.

Release success requires all six [acceptance scenarios](acceptance-scenarios.md), all ten [interface workflows](examples/README.md), bounded malformed-input handling, clean native round trips and an export consumed by a game-engine test project. Style consistency is measured only through palette, dimensions, alpha policy and registration; subjective style still requires visual review.

Planning performance acceptance is practical, not a benchmark claim: on each documented reference machine, rendering/exporting a 64×64, 64-frame, eight-layer asset must finish within 2 seconds after load in a release build; 20 32×32 variants within 5 seconds. Measure before making release claims. Larger jobs are bounded by work limits, not unsupported real-time guarantees.

## Exact selected release

| Group | Included | Why it earns its cost |
|---|---|---|
| Raster | RGBA8 pixels; deterministic line, rectangle, ellipse and polygon rasterization; iterative flood/pattern fill; copy/crop; translation; mirror; quarter-turns; integer nearest scale; source-over and replace composition | Basic creation and precise correction; algorithms shared by sprites, frames and tiles |
| Editing | Named assets/layers/frames/cels; cel offsets; asset pivot and per-frame named anchors; explicit cel copy and copy-on-write frame clones; binary image masks; region edits; independent armor/weapon edits | Component reuse and correction without semantic segmentation claims |
| Palette/style | Shared ordered RGB palettes; exact color replacement; nearest palette mapping; Bayer4 dithering; explicit alpha threshold/key cleanup; persisted collection style profiles | Consistent imports and collections; no second indexed raster engine |
| Effects | Alpha/color masks; four/eight-neighbor dilation and erosion; outer outline; palette ramps as explicit maps; shadow via mask + paint + stamp; seeded sparse texture | Useful composition without an effect graph |
| Animation | Named frames with duration; named ordered clips with direction and playback; layer-preserving frame clone; repeated targeted edits; frame PNG/contact/difference/onion previews; fixed-grid sheet + padding/extrusion + explicit metadata | Animation and existing-asset editing are first-class |
| Reuse/batch | Standalone component assets; native asset transfer; stamp/copy; scalar recipe parameters; bounded flat repeat; explicit seeded texture and variation recipes | Practical asset families without a general-purpose language |
| Tiles | Tiles as frames in a tile asset; rectangular tile grids; edge labels; declared edge-strip comparison; seam previews; explicit transitions; collision rectangles and walkable flags | Enough to construct, validate and use a functional supplied tileset |
| Interoperability | PNG reading of all legal 8-bit and sub-byte grayscale/indexed types, including Adam7; PNG RGBA/indexed output; raw RPX snapshot surfaces; PNG sheet slicing; RGB palette text/GPL; deterministic JSON export | Existing artwork is necessary, despite PNG implementation cost |
| Control | One typed command surface used by single CLI operation and batch recipes; built-in help/JSON command discovery; inspect, dry-run, validate; immutable snapshots and export directories | Low invocation overhead, durable edits and actionable failures |
| Quality | Dimensions/palette/alpha/bounds/frame/anchor/sheet/edge/metadata checks; pixel diffs and histograms; deterministic corpus and format tests | Technical readiness without judging art |

## Excluded from this release

No windows, drawing canvas UI, mouse tools, timeline UI, hotkeys, themes, interactive dialogs, audio/video, brush pressure, GPU, neural model, model API clients, network server, MCP implementation, dynamic plugins, external scripting runtime, skeletal rigging, IK, interpolation that invents poses, semantic image decomposition, natural-language interpreter, unlimited loops or expression evaluator.

No automatic fantasy generator, style classifier, anatomical validator, automatic inferred background removal, automatic edge repair or automatic generation of unseen directions. A supplied flat image has one editable cel until an agent supplies masks or separated components.

## Deferred, with explicit admission criteria

| Capability | Smallest current alternative | Admit later only when |
|---|---|---|
| Arbitrary-angle rotation and advanced pixel-preserving scaling | Quarter-turn + supplied poses; exact nearest integer scale | A representative corpus shows a recurring production need and an algorithm has reviewed deterministic semantics and license |
| GIF/APNG | PNG frames/contact sheet + timings JSON | Consumers demonstrate an animation-preview gap worth another codec |
| ASE/ORA/PSD native import | Export layers/frames from authoring tool to PNG; manifest imports | Loss of layer workflows repeatedly blocks adoption; not mandatory at runtime |
| 16-bit PNG/HDR/color management | Exact stated supported PNG subset; externally supply 8-bit PNG | Actual source assets need it; precision/profile policy is specified first |
| Auto palette extraction/error diffusion/advanced shading | Supplied palette, nearest and Bayer4, explicit ramps | Existing quality corpus fails materially and algorithm cost is justified |
| Layer groups/live effect stack/linked global components | Flat named layers, recipes and bounded multi-target edits | Repeated edits cannot be expressed clearly; persisted behavior contract exists |
| Autotile solver/Wang generation/isometric/hex | Authored rectangular tiles and edge labels; external layout | Concrete engine integration needs topology-specific rules |
| Atlas packing/trim | Fixed grid, untrimmed transparent padding, explicit pivot | A real texture-budget constraint justifies packing and offsets |
| GUI/engine-specific adapters | Library core, PNG/JSON contract | A specific consumer is selected; no dormant interface framework now |

The collection's external generator, optional source editor and final game engine are workflow choices, not executable dependencies of raipixel.
