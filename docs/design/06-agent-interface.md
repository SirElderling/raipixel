# 6. Agent interface specification

Normative interface v1. Examples are in [the workflow catalogue](examples/README.md). The minimal interface is a hybrid: typed individual statements and the same statements in batch files, plus a restricted declarative snapshot. No natural-language interpretation.

## CLI surface

```text
raipixel help [operation] [--json]
raipixel version [--json]
raipixel run RECIPE --out NEW_DIRECTORY [--project SNAPSHOT] [--param NAME=VALUE]... [--export PRESET]... [--dry-run]
raipixel op SNAPSHOT OPERATION KEY=VALUE... --out NEW_DIRECTORY [--export PRESET]... [--dry-run]
raipixel inspect SNAPSHOT [--asset NAME] [--frame NAME] [--layer NAME] [--region X,Y,W,H] [--pixels]
raipixel validate SNAPSHOT [--asset NAME] [--checks NAME,NAME...] [--style NAME]
raipixel export SNAPSHOT --preset NAME --out NEW_DIRECTORY [--dry-run]
```

Global limit flags: `--max-memory BYTES`, `--max-work VISITS`, `--max-input BYTES`; they may lower domain ceilings only. No implicit cwd scanning, environment-based settings, config directory, external execution or networking. `run` without --project starts empty. RECIPE `-` reads stdin under the same size cap. Each `op` argument is one token of the same statement language; shell quoting is the caller's responsibility. CLI field lists use the same bracket syntax, not bespoke repeated options.

`inspect` defaults to full structural summary with counts, not every pixel/histogram for every frame. Selecting asset+frame includes rendered histogram/bbox; adding layer inspects that cel. Region selects a subrectangle; --pixels requires region<=4096 pixels. Frame requires asset; layer requires both asset and frame. No mutation in inspect/validate/help/version. `help --json` emits every operation, keys, types, defaults, constraints, whether it mutates, supported version and feature ID.

`run` saves a new snapshot; requested preset exports are inside `artifacts/PRESET/`. `export` produces only a new export directory; no snapshot. In dry-run, read inputs, execute, render and validate normally, compute intended file paths and lengths, but never create files/directories; stdout lists the planned outputs. It is a correctness check with real work cost, not a promise that later filesystem publication will succeed.

## Grammar

UTF-8, LF or CRLF. Header first non-comment line `raipixel 1`. Full-line comments begin `//`; no inline comments. Blank lines ignored. Keys separated by ASCII spaces/tabs; no whitespace inside unquoted lists. Indentation is cosmetic. Operation/key names ASCII kebab-case. Quoted values may contain spaces and Unicode. Colors are atoms, not comments.

```ebnf
recipe    = header, newline, { statement, newline | repeat-block | comment | blank } ;
header    = "raipixel", space, "1" ;
statement = operation, { space, key, "=", value } ;
repeat-block = "repeat", space, "count=", scalar, space, "var=", identifier,
               newline, { statement, newline | comment | blank }, "end", newline ;
value     = scalar | list ;
list      = "[", [ value, { ",", value } ], "]" ;
scalar    = atom | quoted ;
quoted    = '"', { UTF8-character-except-quote-backslash-newline | escape }, '"' ;
escape    = '\\"' | '\\\\' | '\\n' | '\\r' | '\\t' ;
```

atom contains no whitespace, brackets, comma, equals or quotes. Integers are decimal; signed where stated; no plus prefix, hex integer, exponent, NaN or float. Boolean exactly true/false. RGB exactly #RRGGBB; RGBA #RRGGBBAA or RGB with alpha=255; hex case-insensitive on input, uppercase on canonical output. Null/implicit current target are absent. Lists nest at most four levels, total scalar count at most 1,000,000, with work/memory ceilings still applying. Repeat bodies cannot contain repeat/param/snapshot; nesting fails.

`param name=NAME default=SCALAR` declares a scalar recipe parameter before executable statements. Without default it is required. CLI --param overrides it; unknown/duplicate/unbound parameters fail. `${NAME}` interpolates inside scalar values only; in bare values, the resulting entire value must pass its type parser. No interpolation in operator/key tokens, and values are never reparsed into statements or lists. Repeat var is an integer 0..count-1, may not shadow a param or another binding. Examples `name=weapon-${i}` and `seed=${i}` are valid; `${i+1}` is invalid. Newlines introduced by a string parameter remain string content, never code. Native snapshots forbid all parameters/repeats.

All keys are explicit; duplicates/unknown keys fail. Parse the complete recipe before executing. Unknown op/key errors include closest known spelling when unambiguous, without silently correcting it. Use one typed Operation enum and ordinary match dispatch. Parser descriptors supply help and key constraints; they do not create a plugin framework.

## Operation registry

This table defines allowed fields. `!` means required; omitted optional fields take the stated default. Common pixel-write fields are `region=[x,y,w,h]`, `mask=CEL`, `clip=false`; mode defaults to copy for draw/fill and over for stamp. They apply only to draw/fill/stamp/replace/quantize/alpha/texture. A mutation returns touched-pixel count and explicit clipped count. Empty selection is a successful zero-change operation.

| Statement | Fields, types and defaults |
|---|---|
| param | name! identifier; default optional scalar; recipe-only |
| palette | name!; exactly one of colors=[RGB,...] or file="PATH"; replace=false |
| style | name!; size optional [w,h]; palette optional name; alpha=any (any/binary/opaque); anchors=[] names; nonempty=false; replace=false |
| asset | name!; size! [w,h]; palette/style optional names; pivot=[0,0] |
| import-asset | project! "SNAPSHOT_PATH"; source! asset name; name! destination asset; copies only referenced palette/style dependencies; no replacement; conflicts E_CONFLICT |
| asset-copy | source! asset; name! new asset; clones complete state, Rc-shared pixels |
| asset-set | name!; one or more of palette/style/pivot/clear; clear=[palette,style] allowed (subset, no simultaneous set/clear of same field); values must name existing objects |
| layer | asset!; name!; visible=true; opacity=255; order optional insertion index (default append) |
| layer-set | target! ASSET/LAYER; one or more of visible/opacity/order; reorder index is final position after removal |
| frame | asset!; name!; duration=100; source optional frame name in same asset; cloned image/anchors; explicit duration overrides copied duration, omitted duration uses source timing |
| frame-set | target! ASSET/FRAME; duration! integer |
| cel | target! CEL; size optional (default canvas); offset=[0,0]; source optional CEL; surface loader-only ID (mutually exclusive with source/size, forbidden in recipes); replace=false; with source, size omitted and offset omitted means source values |
| cel-set | target! CEL; offset! [signed-x,signed-y] |
| import | target! CEL; file! "PATH"; offset=[0,0]; replace=false; input PNG or RPX; imported dimensions preserved |
| import-sheet | asset!; layer!; file!; size! [w,h] equal canvas; names! [frame,...]; durations optional list (default 100 each); columns!; padding=0; replace=false |
| draw | target!; shape! pixel/line/rect/ellipse/polygon; color! RGBA; pixel uses at!, line uses points! exactly2, rect/ellipse use rect!, polygon uses points! 3..4096; filled=true for rect/ellipse/polygon; mode=copy; common fields |
| fill | target!; at! [x,y]; exactly one of color RGBA or pattern CEL; phase=[0,0]; mode=copy; common fields |
| stamp | target!; source! CEL; at=[0,0]; rect optional full source; mode=over; common fields |
| move | target! CEL; by! [signed-x,signed-y] (relative offset) |
| transform | target! CEL; op! flip-x/flip-y/rotate-90/rotate-180/rotate-270/scale/reduce/crop; scale uses factor!; reduce uses grid! [gx,gy], phase=[0,0]; crop uses rect!; no unrelated fields |
| replace | target!; colors! [[FROM,TO],...]; preserve-alpha=false; common fields (except mode) |
| quantize | target!; palette!; dither=none (none/bayer4); alpha=preserve (preserve/binary); threshold=128 if binary; phase=[0,0]; common fields (except mode) |
| alpha | target!; mode! threshold/key; threshold! for threshold; color! for key; match=rgb (rgb/rgba) for key; common fields (except mode) |
| mask | source! CEL; target! CEL; mode=alpha (alpha/color); threshold=1 for alpha; color! and match=rgb/rgba for color; replace=false; copies source dimensions/offset; predicate true becomes opaque white |
| morph | target! CEL; mode! dilate/erode; radius=1; connectivity=4 (4/8); binary mask required |
| paint-mask | target! CEL; color! RGBA; binary mask required; true pixels replaced with color, false transparent |
| outline | source! CEL; target! CEL; color! RGBA; radius=1; connectivity=4; replace=false; dimensions/offset source's; crops outer expansion to those dimensions |
| texture | target!; seed! u64; colors! [RGBA,...]; density! integer0..1000; common fields (except mode) |
| anchor | target! ASSET/FRAME; name!; at! [x,y] canvas grid-intersection; replace=false |
| clip | asset!; name!; frames! [frame,...]; direction=none; playback=forward (forward/reverse/pingpong); repeat=0; replace=false |
| tileset | asset!; size! [w,h] equal canvas; edge-width=1 (1..16 and <= both dimensions); strip-equality=false |
| tile | target! ASSET/FRAME; edges! [north,east,south,west] labels; collision=[] list of [x,y,w,h]; walkable=true; replace=false |
| map | name!; asset! tile asset; size! [columns,rows]; cells! [[frame,...],...]; replace=false |
| meta | scope! project/asset/frame; target! for asset/frame; key! identifier; value! quoted/string scalar; replace=false; project has no target |
| preset | name!; kind!; kind-specific fields below; replace=false |
| delete | kind! asset/layer/frame/cel/clip/anchor/palette/style/map/preset/meta; target! typed name/ref except project meta; clip=ASSET/CLIP, anchor=ASSET/FRAME/ANCHOR; meta uses scope! project/asset/frame, key! and target for asset/frame; cascade=false only for layer |
| surface | id! identifier S000000 etc; file! snapshot-relative RPX path; loader-only, immediately after structural declarations and before cels |
| snapshot | version! 1; semantics! 1.0; loader-only declaration immediately after header |

No implicit layer duplication operation: add layer then copy cels with `cel source=...`; no generic “apply pipeline” operation: consecutive statements compose. `asset-copy` adjusts qualified internal references to new asset; map/external references stay unchanged. `frame source` copies anchors; duration omission uses source, while explicit duration is honored. A cel source copies pixels and offset; explicitly supplied offset wins. `import` is replacement, not composition; `stamp` composes.

`replace preserve-alpha=true` requires RGB FROM/TO entries (six hex digits), compares RGB only and retains original alpha. false requires RGBA interpretations and replaces whole pixels. Mask input predicate must produce binary canonical pixels, but paint-mask output may have partial alpha; further morphology then rejects it. Outline is cropped to source dimensions: provide transparent margins or enlarge the cel before outlining; it never silently grows the asset.

### Presets

Common preset fields: name!, kind!, png=rgba (rgba/indexed), scale=1, allow-clip=false, components=false. kind-specific fields:

| kind | Required selection | Optional fields / resulting artifacts |
|---|---|---|
| frame | target=ASSET/FRAME! | image.png + metadata.json |
| sheet/contact | targets=[ASSET/FRAME,...]! | columns=1; padding=0; extrude=0; clips=[] qualified ASSET/CLIP; sheet.png + metadata.json; contact adds registration/bbox stats, no image labels/fonts |
| frames | targets=[ASSET/FRAME,...]! | numbered individual PNGs + metadata.json; no columns/padding/extrude |
| onion | asset!, frames=[previous,current,next]! exactly3 | image.png + metadata.json; selected previous/next may equal current if endpoints intended |
| diff | targets=[ASSET/FRAME,ASSET/FRAME]! exactly2, equal sizes | image.png + stats.json; changed pixel yellow opaque, unchanged transparent |
| seam | target=ASSET/FRAME! tile | repeat=[3,3] default, sides comparison and seam.png + stats.json |
| map | target=MAP! | map.png + metadata.json |
| palette | target=PALETTE! | format=pal (pal/gpl); palette file; png/scale/components/allow-clip forbidden |

components=true only for frame/sheet/frames/contact: export each existing cel as its own PNG scaled by the preset integer scale and record scaled and logical asset-space offset, layer visibility/opacity, canvas/pivot and qualified frame. Rendered base sheet still uses visible layers only. PNGs use names derived from stable numeric entry/layer indices, not user metadata paths. clips allowed only for sheet/frames/contact. Extrusion allowed for sheet only (contact uses transparent padding). `components=true` does not reconstruct native projects from game JSON automatically.

Selected targets cannot be duplicated, except onion/diff where duplicate endpoints are meaningful. Repetition in clip playback metadata remains valid. If columns exceeds target count fail, rather than invent empty columns. Palette presets ignore frame style. Asset style and optional explicit validation style both apply; conflicting constraints fail clearly.

## Execution, files and errors

Input recipe paths resolve relative to the recipe file's parent; stdin/op resolves relative to explicit invocation cwd. Native image paths resolve within snapshot. Output --out is OS-native path, but recipe paths are UTF-8. Non-UTF-8 recipe paths fail with a hint to rename/pass through a supported path; never use lossy conversion. Canonical metadata keeps optional provenance as a string only.

NEW_DIRECTORY must not exist. Create a uniquely named sibling staging directory with create_dir retry; source project never modified. Write and flush files, then rename the stage to the destination on the same filesystem. No --force/in-place replacement in v1. On failure remove the owned stage best-effort; report leftover stage if cleanup fails. Atomic visibility of a new directory is the intended publication primitive; power-loss durability is not guaranteed without platform-specific fsync of directories. Portable std cannot fully prevent hostile symlink/rename races: callers must own and trust the output parent. Refuse known existing destination or symlink and recheck before rename, but do not call this a filesystem security sandbox.

Within a snapshot, accept only regular files under the validated manifest-relative image path; reject symlinks and path traversal. Recipe imports are explicitly authorized paths and may be symlinks to source images; read once into bounded bytes, and save resulting pixels. No path from metadata is executed or used as an output filename. Windows reserved names/case collisions cannot occur in generated numeric paths. Snapshot and export image filenames are ASCII; user-readable IDs stay in manifests/JSON.

The transaction commits at successful directory rename. An stdout/stderr write or broken-pipe error after that point may return E_IO/exit4 although the directory is committed; never delete it. Report publication=committed and path where the remaining output channel permits. Callers check destination existence/manifest after transport failure before retrying. All-or-nothing applies to processing/publication errors before commit.

Successful machine operations write one schema-1 JSON document to stdout and newline; stderr empty. Warnings live in stdout diagnostics. Failure writes one JSON error to stderr, stdout empty; help/version plain text unless --json. Broken pipe is I/O failure; no half JSON promise if the OS cuts the output stream. No progress chatter from the executable.

```json
{"schema":1,"ok":false,"error":{"code":"E_BOUNDS","operation":"draw","line":12,"column":1,"target":"hero/armor/walk1","message":"Rectangle exceeds cel width","details":{"width":32,"rect":[30,0,4,4]},"hint":"Use a rectangle within the cel or explicitly set clip=true"}}
```

Stable top-level success fields: schema, ok, semantics, operation, diagnostics and result. Stable diagnostics: severity, code, target (when applicable), message, details, hint; parse location line/column plus repeat_iteration when relevant. Do not include arbitrary input bytes, full artwork or secrets in error messages. Work/limit errors include requested/allowed and affected target. Human messages may evolve; code and typed details are the agent contract.

| Exit | Meaning | Error families |
|---|---|---|
| 0 | Success, including no-op and dry-run | Warnings remain diagnostics |
| 2 | CLI/recipe/type/reference error | E_USAGE, E_PARSE, E_PARAMETER, E_ARGUMENT, E_REFERENCE, E_EXISTS, E_CONFLICT, E_VERSION |
| 3 | Technical validation failure | E_STYLE, E_PALETTE, E_MASK, E_ANIMATION, E_TILE, E_SHEET, E_PROJECT, E_BOUNDS |
| 4 | I/O or unsupported/corrupt format | E_IO, E_OUTPUT, E_PNG_SIGNATURE/CHUNK/CRC/FILTER/DEFLATE/ADLER/DEPTH/ANIMATED, E_RPX |
| 5 | Resource ceiling/allocation failure | E_LIMIT, E_ALLOC |
| 70 | Detected internal invariant defect | E_INTERNAL; no deliberate panic for user input |

validate returns a schema-1 report on stdout for both pass/fail and exit 3 on failed checks, because diagnostics are its result; other validation-failing commands use stderr error envelope. All requested checks run before returning validate. `--checks` names: structure, dimensions, palette, alpha, bounds, animation, registration, sheet, edges, metadata. Default structure/dimensions/palette/alpha/bounds/metadata plus relevant animation/edges for selected objects. `sheet` requires a preset selection internally (checked on export); standalone validate reports persisted sheet presets. `registration` requires shared anchors across requested frames; if no style-required anchor names exist, compare common names and warn that coverage is limited.

## JSON contracts and compatibility

Export declares alpha_mode="straight" and color_policy="unmanaged-encoded-rgb8". Component scale/offset follows preset scale and includes logical fields.

Output-only JSON handles quotes, backslashes, control characters, valid UTF-8 and integer numbers. No input JSON parser. Ordered object keys are emitted in schema order; named maps sorted lexicographically; layer/frame/clip sequence arrays retain domain order; color histogram sorted RGBA. Checksums use CRC32 over canonical rendered RGBA bytes; label them non-cryptographic. Unicode metadata escaped only where JSON requires; invalid UTF-8 input is an error.

Export schema includes schema=1, semantics="1.0", images (filename,width,height), frames (asset,frame,index,image,rect,canvas,logical_canvas,pivot,logical_pivot,anchors,duration_ms), clips (asset,name,direction,playback,repeat,sequence indices), palettes/styles used, tiles (qualified identity,edges,collision,walkable), maps and optional components. Anchors/collision/rect/pivot are output-pixel integers after scale; logical fields retained for consumer reasoning. Coordinate origin explicitly top-left. Canvas is untrimmed, and spritesheet frame rectangle excludes padding/extrusion. No compatibility claim is made for any third-party editor-specific metadata schema.

Recipe/snapshot major version changes for breaking syntax/state; engine semantics changes for algorithm output differences; JSON schema major for breaking fields. Unknown versions fail with supported-version hint. Unknown fields fail on input. Readers may ignore additive JSON output fields; stable codes remain stable within schema major. Migration is an explicit future command when needed, not silently loading changed raster semantics. Old snapshots store pixels and can still be inspected with a compatible reader even when newer recipe algorithms differ.

Canonical snapshot order: header, snapshot declaration, sorted palettes/styles, sorted assets with ordered layers/frames, numeric surfaces in first-encounter order, cels by asset/layer/frame traversal, anchors, clips, tile/maps, metadata and presets. Snapshot palettes contain colors, never file imports; every cel uses surface, never source/import. Internal surface IDs and RPX names are assigned together. The loader accepts only palette/style/asset/layer/frame/surface/cel/anchor/clip/tileset/tile/map/meta/preset declarations after snapshot, in creation mode; frame source, palette file and all replace=true fields are forbidden; it forbids all mutation/import/param/repeat commands and external paths, and validates references after the complete manifest.

## Normative success result shapes

The following are shape examples, not results from executing an application. All indicated counts/coordinates are nonnegative JSON integers unless a signed offset/pivot is documented. Fields in these examples are required for the selected operation; optional target selections add described fields without changing their meaning.

Mutation/run result contains operations (expanded executed statement count, excluding header/params), visited_pixels (candidate pixel-write visits, repeated visits counted), changed_pixels (write events whose replacement RGBA differs from the pre-write value; repeated changes of the same pixel are counted), clipped_pixels (explicitly discarded writes), objects_added/deleted, snapshot (destination path), artifacts (relative path and byte length), and publication (committed). Relative offset/metadata changes do not count as pixel writes. Additional work_units reports total charged parser/codec/structural/pixel work. These metrics are operational counts, not final-image distinct differences; the diff preset's changed_pixels is distinct positions in the two final rendered canvases. Transform resize compares writes to overlapping old image-local positions, absent old positions treated transparent; deletion of old cropped pixels is recorded in operation details, not as a new write.

```json
{"schema":1,"ok":true,"semantics":"1.0","operation":"run","diagnostics":[],"result":{"operations":6,"visited_pixels":12,"changed_pixels":12,"clipped_pixels":0,"work_units":180,"objects_added":0,"objects_deleted":0,"snapshot":"hero-armor-v2","artifacts":[],"publication":"committed"}}
```

inspect with asset/frame reports identity, canvas, pivot, duration_ms, layers (name/visible/opacity/cel size/offset or null), anchors (name/at), clips containing the frame, palette/style references or null, metadata, and raster statistics. Statistics fields: pixels, visible_pixels (alpha>0), alpha_zero/alpha_partial/alpha_opaque, bbox (null if no visible pixels, else [x,y,w,h]), histogram ([{rgba:"#RRGGBBAA",count:N}]), crc32 (eight uppercase hex digits, non-cryptographic). With region, stats/pixel lists cover that region and include rect; optional pixels is row-major RGBA string array. Structural summary without frame uses counts/names and omits raster statistics. Null here is JSON output representation only, not recipe syntax.

```json
{"schema":1,"ok":true,"semantics":"1.0","operation":"inspect","diagnostics":[],"result":{"asset":"dot","frame":"base","canvas":[1,1],"pivot":[0,0],"duration_ms":100,"layers":[{"name":"ink","visible":true,"opacity":255,"cel":{"size":[1,1],"offset":[0,0]}}],"anchors":[],"clips":[],"palette":null,"style":null,"metadata":{},"raster":{"pixels":1,"visible_pixels":1,"alpha_zero":0,"alpha_partial":0,"alpha_opaque":1,"bbox":[0,0,1,1],"histogram":[{"rgba":"#FF0000FF","count":1}],"crc32":"D2433660"}}}
```

Dry-run result uses dry_run=true, publication="planned", outputs ([{path:relative artifact path,bytes:N}]), operations/work_units and intended destination. It uses the same serialization/output computation as real execution, so lengths are exact for the requested current job; filesystem permission/space/rename success is not forecast. No temporary directory is created.

```json
{"schema":1,"ok":true,"semantics":"1.0","operation":"export","diagnostics":[],"result":{"dry_run":true,"publication":"planned","destination":"hero-game","operations":0,"work_units":900,"outputs":[{"path":"sheet.png","bytes":1234},{"path":"metadata.json","bytes":567}]}}
```

The byte counts/work count above are illustrative. Actual export must calculate its own sizes; these example numbers are not format benchmarks.

validate result contains passed boolean, checks [name,...], error_count and warning_count. Complete diagnostic objects appear once at the top-level diagnostics array. A registration error details includes anchor name, expected/actual [x,y], baseline_frame and affected frame; missing anchor details contains required name. Diagnostics sort by target/check/coordinate as defined. Previews may create colors outside asset palettes; validation applies to underlying assets, never decorative onion/diff/checker overlays.

Serialized destination paths must be valid UTF-8/Unicode; otherwise CLI rejects before processing with E_USAGE rather than outputting a lossy path. OS-native argument handling preserves paths until this validation. Job success reports errors only before commit; reporting-channel errors after commit follow the documented publication rule.
