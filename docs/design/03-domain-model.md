# 3. Technical domain model

Normative state contract. Foundation implements the minimal static path; Core adds the declared animation/reuse behavior. Later state additions require a format-version decision before implementation.

## One coherent raster model

| Entity | State and relationships |
|---|---|
| Pixel / Image | Straight RGBA8 `[u8;4]`, contiguous row-major image with positive width/height. Preserve RGB samples even when alpha is zero. New transparent pixels are `[0,0,0,0]`. |
| Palette | Project-owned name and ordered entries `{name,rgb}`; 1–256 unique entry names and RGB values. Color references resolve to opaque RGB; explicit RGBA literals supply alpha. Updating a palette does not recolor pixels. |
| Style | Project-owned optional size/palette, alpha policy `any/binary/opaque`, required anchor names and nonempty rule. Technical constraints, not perceptual style. |
| Asset | Name, fixed canvas, integer pivot, nullable palette/style references, ordered layers, ordered frames, clips and metadata. Sprite/icon/component share this type. |
| Layer | Asset-owned name, visibility and u8 opacity. Vector order is bottom-to-top, consistent across frames. |
| Frame | Asset-owned name, duration_ms, cels keyed by layer, named anchors and metadata. Frame vector is default export order. Missing cels are transparent. |
| Cel | Image reference and integer asset-space offset, addressed `asset/layer/frame`. Images may be immutable `Rc<Image>` allocations; edits detach after budget checks. |
| Clip | Asset-owned name, nonempty ordered frame-name sequence, direction label, forward/reverse/pingpong playback and repeat count. References may repeat. |
| Component | Ordinary asset/cel used as a source. Copy/stamp and transfer create independent editable results; no live link, scene graph or rig. |
| Project | Ordered-by-name assets, palettes/styles, string metadata. External source paths are optional inert provenance, never required to reopen. |
| Operation | Finite typed enum obtained from JSON. No persisted closures, program, implicit current target or operation history. |

Advanced rectangular tile state can reuse asset frames plus explicit edge/collision metadata; maps will reference qualified frames. That extension MUST specify typed state, persistence and an established map-interchange profile before delivery. Do not implement dormant map machinery in Foundation.

## Identity, coordinates and references

Names match `[A-Za-z][A-Za-z0-9_-]{0,63}`, are case sensitive, and unique in their owner. IDs contain no slash. A layer or frame reference has two segments, distinguished by operation type; cels have three. Display/provenance text can be Unicode. Numeric image IDs in snapshots are implementation-neutral references, not semantic component identities.

Origin is top-left, x right/y down. Public positions/offsets are signed i32. Pixel positions index cells; pivots and anchors identify integer grid intersections. Rectangles `[x,y,w,h]` are half-open with positive sizes. Pivots/anchors may lie on the closed canvas boundary; off-canvas registration is Deferred until a demonstrated use needs it.

Drawing and masks use cel-local coordinates. Offsets place cels on the canvas without resampling. Crop adds the crop origin to the offset so retained pixels stay in place; flip/quarter-turn/scale retain the top-left offset. Pivots/anchors do not move implicitly. A resize/transform that changes registration requires explicit metadata changes and inspection. Render clips to canvas; strict export rejects nontransparent content outside it unless `allow_clip:true` is requested.

All owners MUST exist before child creation. Creating an existing object fails unless an operation has explicit `replace:true`. Clones copy metadata and immutable image handles; changes never propagate to other cels implicitly. Multi-target operations use explicit nonduplicate lists, not wildcards. Delete refuses referenced objects; deleting a layer and all its cels requires `cascade:true`. Renaming is Deferred; copy and explicit reference changes suffice initially.

## Preservation and equality

An edit MUST preserve every unselected stored pixel sample and unrelated state field, including hidden RGB, timing, anchors, palettes and references. Copy mode copies all four bytes, including transparent clears. Source reads come from the pre-operation state, including overlapping stamps and multi-target source aliases. A batch validates every target before any writes for that operation; the job runner provides whole-job rollback.

Semantic project equality compares pixels and all declared fields, ignoring pointer sharing and numeric surface filenames. A snapshot stores current pixels/state, never only a recipe. Native round trips preserve all editable state and reference relationships; shared-memory identity is not a user-visible relationship. Exporting a flattened frame cannot preserve layers; native snapshots remain authoritative.

## Limits and checked work

Initial default ceilings are engineering safeguards, not feature-count or image-size marketing targets. They may be lowered by CLI; raising them requires measured review. Checked arithmetic MUST precede allocation/traversal, including clipped off-canvas geometry.

| Resource | Initial ceiling |
|---|---:|
| Image/canvas side; image pixels | 4096; 16,777,216 |
| Request/manifest bytes; JSON nesting | 8 MiB each; 64 levels |
| Input file; aggregate project input | 128 MiB; 512 MiB |
| Accounted live payload | 512 MiB |
| Operations; explicit targets per operation | 100,000; 4096 |
| Structural objects; cels; references | 100,000; 250,000; 1,000,000 |
| Stored names/metadata strings | 16 MiB aggregate; individual string 64 KiB |
| Frames/asset; layers/asset; anchors/frame | 4096; 256; 64 |
| Duration; clip entries; repeat | 1–60,000 ms; 16,384; 0–65,535 (0 means infinite metadata) |
| Charged work per job | 1,000,000,000 units |

Charge input/parser bytes, codec symbols/output bytes, checksums, pixel visits, structural/reference copies and metadata cloning. Count temporary buffers, COW detaches, masks, parsed commands, decoder and render scratch in payload estimates; stream staged PNG output rather than buffering it all. Structural/reference limits also bound cheap shared-image amplification. Use bounded i64/i128 geometry intermediates and fallible reservation where available. The payload ceiling is not a process RSS guarantee; std allocation aborts or OS termination remain possible.

## State transition

Load empty or validated snapshot → parse/type-check full request → execute on private working state → validate structure and requested rules → render requested artifacts → stage new snapshot/output directory → publish. Before publication, failure discards working state and owned staging files; source snapshots are immutable. Intermediate visual correction uses a new job/snapshot boundary. Reports/exports in a job observe its final state. No mutable daemon, undo database, autosave service or execution on load.
