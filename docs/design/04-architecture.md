# 4. Architecture and interoperability

## Implementation boundaries

One Cargo package, reusable library and thin CLI; edition 2024, proposed minimum Rust 1.85 using stable APIs (a design floor, not the latest-version claim). Both crate roots MUST forbid unsafe code. No dependencies, dev-dependencies, build-dependencies or build.rs. Cargo.lock MUST contain no third-party package. Std's OS implementation and the normal compiler/linker remain permitted.

| Module boundary | Responsibility |
|---|---|
| model / limits / error | State invariants, references, allocation/work budgets and typed errors |
| raster / ops | Geometry, masks, palette operations, transforms and typed operation execution; no paths or process I/O |
| render / validation | Composition, previews/sheets, statistics, objective checks and ordered diagnostics |
| codec | Independently implemented bounded PNG, zlib/DEFLATE and later explicit format adapters; no persistence decisions |
| json / interface | RFC 8259 parser/emitter, typed request conversion and operation descriptors/schema/help |
| project_io | Data-only JSON manifests and PNG surface load/save; restricted internal paths |
| job / main | File inputs, transaction, CLI paths, reports, exit codes; main delegates graphics to the library |

These are cohesion boundaries, not a fixed file count. A finite enum/match and adjacent argument descriptors are sufficient. No registry framework, dynamic plugins, replacement-backend traits, ECS, universal backend abstraction, event bus, scheduler, indexed raster engine or effect graph. Keep codec internals private behind ordinary module functions using model-owned image/error/limit types. Add a substitution interface only when an actual second implementation requires it. New operations add an algorithm, typed variant/descriptor and meaningful tests. Only new persisted semantics require model/schema changes. The library API is unstable before 1.0.

## Deterministic raster and rendering

Single-threaded initially; independent jobs may run in separate host processes. Use integer geometry/color arithmetic, ordered maps and deterministic target traversal. No floating-point graphics, platform fonts, OS randomness or directory-order dependence. Record engine semantics in requests/results/snapshots. Same semantic inputs and engine semantics MUST yield equal state/pixels; the same writer build/settings MUST yield canonical PNG/JSON bytes across targets. Equivalent independently encoded PNGs need not be byte-identical. Raster algorithm changes require a semantics-version decision.

Render one frame at a time. Start transparent; copy the bottom visible cel with layer opacity applied, preserving its hidden RGB samples. Composite remaining visible layers with source-over in layer order. This explicit base-copy rule makes the ordinary single-layer import/export path preserve all RGBA samples. Other flattened transparent RGB values follow this compositor policy, not hidden samples of occluded layers. Native snapshots preserve every cel sample regardless of visibility.

Let round(n/d) for nonnegative integers be `(n+d/2)/d`. Effective source alpha `a=round(pixel_alpha*opacity/255)`; destination alpha b; `q=a*255+b*(255-a)`. For source-over, a=0 returns the destination unchanged. Otherwise output alpha is round(q/255) and each channel is `round((src*a*255+dst*b*(255-a))/q)`. Copy retains RGB and replaces alpha with a. Wide integer intermediates and ordered composition are mandatory. Encoded RGB byte-space blending is intentional; no linear-light/profile conversion is claimed. Partial alpha/opacity can introduce colors outside a palette, so validate visible rendered frames. Hidden helper masks are not constrained by the asset display palette; inspection may examine their samples explicitly.

## PNG and zlib/DEFLATE: Foundation dependency

PNG import MUST ship in the first static slice. Decode stored, fixed-Huffman and dynamic-Huffman DEFLATE blocks, overlapping LZ77 references and the zlib wrapper; validate CRC32 and Adler32. A stored-only decoder is insufficient for real PNG interoperability. A general compressor is unnecessary initially: write RGBA8, filter None and stored blocks ≤65,535 bytes, with bounded consecutive IDAT chunks. Valid but larger files are an explicit trade-off. Reuse this writer for native surfaces. Exact indexed output is Advanced, not an initial alternate raster backend.

Foundation accepts static PNG legal types 0/2/3/4/6 at 8-bit, types 0/3 at 1/2/4-bit, all filters and both noninterlaced/Adam7 layouts. Reject 16-bit with E_UNSUPPORTED_DEPTH and animation control chunks with E_UNSUPPORTED_ANIMATION; never silently discard animation. This is a documented import profile, not full PNG decoder conformance. Writer output MUST conform to the PNG specification.

Precompute expected scanline bytes, including pass-local Adam7 row sizes, and cap inflate output to that exact count. Validate signature, lengths/order, legal IHDR, PLTE/tRNS constraints, consecutive IDAT, required IEND, chunk CRC, palette indices, filter bytes and zlib checksum. Reset previous row per pass; sub-byte samples are MSB-first and tRNS compares unscaled samples. Reject unknown critical chunks. Recognized color-space/profile chunks are reported as uninterpreted; skipped ancillary payloads are CRC checked and never inflated. Do not copy arbitrary ancillary metadata into edited output or promise color-managed appearance preservation.

Inflater MUST check CM/FCHECK/CINFO/FDICT (dictionary unsupported), stored LEN/NLEN, reserved block/symbol values, Huffman oversubscription and permitted degenerate exceptions, repeats, end code, truncation and distance within both produced data and advertised window (≤32 KiB). Backreferences copy from growing output. Exactly one zlib stream is decoded; PNG-permitted bytes after its checksum in IDAT warn and are ignored. Reject bytes after IEND in this profile. Malformed and unsupported inputs have distinct errors. Test disputed edges against cited specification clauses, never copied decoder code.

## Persistent format decision

**Use a directory with `project.json` and numbered `images/000000.png` surfaces.** The manifest stores the complete editable model; PNG stores exact cel pixels. It MUST be data-only, self-contained, versioned, and reject unknown required fields/versions. Recipes/jobs are separate inputs, never executed when opening a project. No RPX or custom snapshot grammar is retained.

The manifest remains a raipixel schema: PNG, OpenRaster and animated raster files do not jointly express every asset/clip/pivot/anchor/style/reference. Standard containers do not remove the need for that model. Reuse ordinary JSON constructs rather than inventing a binary format. Snapshot canonicalization sorts named maps, preserves ordered layer/frame/clip arrays and numbers images by first encounter in asset-name/layer-order/frame-order traversal. Serialize each shared image allocation once in that traversal; the loader creates one allocation per image ID. Do not content-deduplicate distinct allocations in the baseline. This preserves deterministic save/load bytes without hashing or relying on pointer addresses; allocation sharing itself remains nonsemantic. No hash-addressed cache, journal, ZIP wrapper or database in Foundation.

| Alternative | Assessment / decision |
|---|---|
| Raw RPX + custom manifest grammar | Tiny surface decoder and fast raw I/O, but adds private file format/parser rules, external inspection friction and uncompressed storage. Initial stored-block PNG is likewise mostly uncompressed with small additional framing overhead; no disk-size advantage is claimed. Once PNG import is Foundation, this duplication has no demonstrated benefit. Reject. |
| JSON manifest + PNG directory | One JSON parser and PNG codec already required for commands/interchange; inspectable, independent consumers can read surfaces, complete state preserved by manifest. Stored-block PNG has modest overhead and poor compression; measure before optimizing. Select. |
| OpenRaster as sole native project | Useful layered interchange, but baseline lacks animation/clip/anchor/component semantics; needs ZIP/XML and unsupported blend/group decisions. Select later interchange profile, not native storage. |
| Single archive or custom compressed native file | Fewer files but more container/atomic-write complexity. Defer until measured project/file-count problems justify it. |

## Interchange profiles

[Research](01-research.md) links formal standards. The baseline promise is exact native state round trips and sample-preserving supported PNG import/export for the ordinary single-layer path. It is never source-file-byte preservation.

| Format | Classification / actual semantics | Decision and preservation boundary |
|---|---|---|
| PNG | W3C raster specification; no named editable layers/pivots; current spec also defines APNG | Foundation static import profile + conforming RGBA writer. Full 16-bit/color management Deferred. |
| JSON / zlib / DEFLATE | Published RFC interchange/compression specifications | Implement required grammars/codec behavior independently. JSON application fields are explicitly versioned raipixel schemas. |
| OpenRaster | Published open layered-image specification, ZIP + stack.xml + PNG | Advanced flat normal-blend layer import/export with offsets/visibility/opacity and mandatory merged image/thumbnail/mimetype. No baseline animation promise; animation proposals are not baseline. |
| APNG | Animation in PNG Third Edition: frames/rectangles, blend/disposal and fractional delays | Advanced export of full-frame rendered animation with SOURCE/NONE and explicit loop/timing mapping. Not editable-layer interchange; no native state round trip. Import Deferred until disposal and rational timing preservation are specified. |
| Documented editor-native binary formats | Vendor/project-specific specifications; can describe layered cels, linked cels, palettes, timing, tags, slices and sometimes tilemaps | Broad read/write Deferred. Each format needs a demand-backed subset and explicit rejection/preservation ledger; documented does not mean open standard. No generic editor-native compatibility claim. |
| Documented JSON/XML tile-map formats | Ecosystem-specific, including global tile IDs, flags, tilesets, animation and object layers | Advanced finite orthogonal JSON-array profile before a new map interchange convention. Base64/compressed, infinite, hex/isometric and engine-specific semantics Deferred. |

OpenRaster import MUST reverse top-to-bottom stack order into internal bottom-to-top order. The proposed profile rejects nested groups, unsupported blends/effects, external/non-PNG layers, unknown rendering extensions and unrepresentable opacity, rather than silently flattening. For opacity0/1, conversion is exact. Other input opacities map to u8 by exact-decimal nearest rounding (ties upward), require explicit conversion acceptance and report layer_opacity in fidelity.converted; a source-semantic lossless request fails if it cannot be represented exactly. Export intermediate u8 opacity as n/255 rounded to nine decimal places so reimport recovers n; this promises native u8 round-trip, not preservation of every external decimal opacity. Layer samples/offsets/order/visibility remain exact. Independently rendered output may differ through rounding/compositing policy and must be compared rather than promised bit-identical. Do not claim full baseline support for this subset. ZIP needs STORED/DEFLATED reads, CRCs, central/local-header agreement and expansion/path bounds; reject encryption, multi-disk and out-of-profile ZIP64. XML must follow a specified supported UTF-8 XML profile, reject DTD/external entities and never fetch external resources. Final syntax and independent fixtures are a gate before delivery, not a Foundation implementation commitment.

APNG export can exactly encode integer durations ≤60,000 ms as reduced fractions of seconds. A clip's finite playback sequence is expanded once; repeat=0 maps to infinite plays, positive repeat to total plays. Export only one selected clip. No claim to recover original partial rectangles/disposal from rendered frames. PNG+JSON frames remain the authoritative timing/registration interchange.

## Cross-format fidelity contract

Interoperability MUST be demonstrated as conversions, not a list of recognized extensions. The Foundation/Core gate is PNG → native editable project → corrected PNG, plus complete native round trip and independently consumed frame/sheet metadata. The first planned Advanced interoperability increment is OpenRaster → native layered state → OpenRaster/PNG, followed by rendered animation → APNG. Optional tiles/map interchange follows an established documented profile, never a new proprietary map convention merely for convenience.

Each conversion MUST identify preserved, omitted, converted and unsupported semantics in machine-readable `fidelity` output and diagnostics. A lossless request fails if required state cannot be represented; an explicit single-frame/raster export declares omitted edit structure rather than pretending to round-trip it. No opaque preservation bucket or private PNG/APNG/OpenRaster extension is used to hide native state. The native manifest is a publicly specified raipixel schema over standard JSON/PNG, not an external standard or substitute for format interoperability.

For PNG, verify exact decoded samples in the stated profile; for native state, compare every semantic field/reference; for OpenRaster, compare the supported layer subset and separately compare rendering; for APNG, compare rendered frame order, durations and loop metadata while declaring omitted layers/anchors/clips. Formats without those semantics cannot promise them. A source profile/rendering feature outside the contract must be rejected for lossless conversion or reported with an explicit conversion choice. See acceptance scenario E for independent-consumer gates.

## Files, publication and portability

Process explicit inputs; no networking, subprocess or implicit filesystem scanning. Native paths must be normalized relative numeric surface paths beneath images, regular files without symlinks/traversal; metadata cannot redirect I/O. Input image paths are explicit reads and may resolve through caller-controlled symlinks. Read bounded input once. Generated output filenames are ASCII and portable; serialized CLI paths must be valid Unicode, not lossy conversions.

Create a unique sibling staging directory in an owned output parent, write/flush, then rename to a previously absent destination. Never modify the source or silently overwrite an existing output. Remove owned staging best-effort after pre-commit failure. Rename is the commit boundary; an stdout/broken-pipe error afterward MUST retain the published result. Callers check destination after transport failure before retrying. Std alone cannot guarantee hostile-parent race protection or power-loss durability; this is a trusted local-workspace transaction, not a filesystem sandbox.

Cross-platform tests start in Foundation. Measure stable release profiles only after correctness; no hard binary-size target, mandatory static linkage or unmeasured size estimate. Normal OS/toolchain facilities are allowed, but production Rust declares no FFI and spawns no executable.
