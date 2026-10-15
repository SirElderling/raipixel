# 5. Functional specification and prioritization

Normative behavior for proposed capabilities. Nothing is implemented. Foundation and Core are the first useful toolkit; Advanced/Deferred/Excluded are not release requirements. IDs F01–F25 retain traceability, with subfeatures separated where their value/cost differs. [Interface](06-agent-interface.md) owns field names; [architecture](04-architecture.md) owns codecs/rendering/storage.

## Requirement-by-requirement disposition

| ID | Foundation | Core | Advanced | Deferred / Excluded and reason |
|---|---|---|---|---|
| F01 Documents | Named assets/layers, one-frame path, cels, exact save/load | Frame clones/order, styles and complete animation state | — | Live undo database Deferred; snapshot iterations suffice |
| F02 Drawing | Symbolic grids (new explicit authoring requirement), pixels/lines/rectangles | Ellipse/polygon, multi-target writes | Brush-pattern conveniences | Interactive pressure/freehand UI Excluded |
| F03 Fill | Exact four-neighbor flood fill | — | Repeating pattern fill with explicit phase | Inferred semantic regions Deferred |
| F04 Copy/composition | Exact copy and defined source-over | Batched stamps/masked composition | — | General blend/effect graph Deferred |
| F05 Transforms | Integer placement | Mirror, quarter-turn, crop, integer scale, explicit-grid reduction | Grid candidate inspection | Arbitrary-angle/pixel-art interpolation and irregular-grid reconstruction Deferred |
| F06 Targeted edits | Named cels, explicit regions, unrelated-state preservation | Binary masks, multi-frame component correction | — | Automatic part extraction/segmentation Deferred |
| F07 Palettes | Ordered named RGB entries, exact extraction, simultaneous recolor | Palette/style references, palette-controlled variants | Ramp conveniences | Palette update never silently recolors; subjective style inference Excluded |
| F08 Quantize/dither | — | Nearest palette mapping, alpha preserved | Bayer4 ordered dither | Error diffusion/perceptual clustering Deferred pending corpus |
| F09 Alpha | Explicit exact key/threshold within selected region | Mask-limited cleanup | Border-connected key cleanup | Semantic background inference Deferred |
| F10 Masks/morphology | Rectangular selection | Binary masks from alpha/color, editable as ordinary cels | Four/eight-neighbor dilation/erosion | Smart-selection vision Deferred |
| F11 Effects | — | Shadows/ramp shading via mask + recolor/stamp | Explicit outer outline and morphology effects | Automatic lighting/texture models Deferred |
| F12 Procedural | Grids/shapes/composition | Versioned seeded sparse texture, symmetry/repetition via stamps/transforms | More bounded typed pattern operations | Category-specific synthesis Deferred; anatomy/pose inference guarantee Excluded |
| F13 Reuse/batches | JSON operation arrays | Explicit target arrays, JSON job templates, asset clones/variants | More high-level operations only with measured savings | Custom loops, parameters, expression language and script runtime Excluded |
| F14 Animation | State accommodates duration/anchors; no animation delivery claim | Frames, clones/order/delete/timing/clips/directions/anchors, supplied poses | Explicit rigid-motion helpers | Skeleton/IK/live rig Deferred; inventing missing art Excluded |
| F15 Inspection previews | Frame PNG, bounded pixel/symbol views, cross-snapshot diff statistics | Contact/onion/diff PNG, timing/registration reports | Additional diagnostic overlays | Real-time GUI playback Excluded |
| F16 Tiles/maps | — | Assets can contain tile artwork as ordinary frames | Rectangular tiles/maps, authored transitions, edge labels, collision rectangles/walkable metadata | Autotile solver, Wang synthesis, isometric/hex/navmesh Deferred |
| F17 Seam checks | — | — | Repeat mosaic, actual-neighbor label checks, optional exact-strip contract | Automatic visual seam approval/repair Deferred |
| F18 Validation | Structure/dimensions/palette/alpha/bounds, preservation comparisons | Timing/clips/registration/style/sheets | Tile/edge checks | Artistic quality classifier Excluded |
| F19 Inspection | Structure, histogram/bbox/alpha, exact region pixels/symbols | Frame/layer/clip/component details | Grid candidates, collection summaries | Unbounded full-project pixel dumps Excluded |
| F20 PNG | Real static import, full required DEFLATE, RGBA8 output | Same profile and guarantees | Exact indexed output; APNG export is separate | 16-bit/HDR/color management and APNG import Deferred |
| F21 Sheet input | — | Explicit regular-grid PNG slicing, frame names/durations | More documented layout profiles | Inferring arbitrary packed sheets Deferred |
| F22 Game export | Frame PNG + explicit JSON | Untrimmed frames/sheets, padding/extrusion, clips/registration/component metadata; reusable request files | Established map export profile | Atlas packing/trim/rotation Deferred |
| F23 Persistence | Complete data-only JSON + PNG snapshots | Complete Core editable state | Versioned later state/extensions | RPX/custom manifest grammar discarded; archive packaging Deferred |
| F24 Palette exchange | Palette JSON uses same entry representation as commands/manifests | Reuse/extract within projects | Documented text-palette interchange if needed | Extra custom .pal grammar discarded; legacy palette formats Deferred |
| F25 Native transfer | — | Copy one asset and referenced palette/style dependencies from another snapshot | — | General project merge/live external references Deferred |

Layered-image interchange is **Advanced and planned**, with the OpenRaster profile in architecture. It is distinct from F23 native preservation. Documented editor-native formats are individually Deferred; they are not open standards merely because their specifications are published.

## Creation, editing and pixel invariants

A symbolic grid maps one printable ASCII character per pixel to a declared color or palette entry. All rows have the same nonzero byte length. A `skip` symbol leaves destination samples untouched; a transparent color writes transparency. Grid payloads support entire images and small corrective patches with one operation. Do not expand grids into individual command objects.

Every pixel-writing operation takes explicit targets and optional local `region`, binary `mask`, and `clip` only where the interface declares them. Coverage intersects region and mask. Mask dimensions MUST match the target cel; offset is ignored for local selection. Writes outside the image fail unless clip=true, in which case discarded coverage is counted. An empty effective selection succeeds without changes. Preflight geometry/work before clipping so distant endpoints cannot cause unbounded traversal. Repeated or missing targets fail.

Line coverage uses integer Bresenham: canonicalize endpoints in lexicographic (x,y) order; dx=abs(x1-x0), dy=-abs(y1-y0), err=dx+dy; save e2=2*err per step, update x when e2>=dy and y when e2<=dx. Include endpoints. Rectangles use half-open coverage; outlines select boundary pixels. Filled ellipse in rect [rx,ry,w,h] selects centers satisfying `(2*x+1-2*rx-w)^2*h^2 + (2*y+1-2*ry-h)^2*w^2 <= w^2*h^2` in i128. Its outline selects inside pixels with a four-neighbor outside. Filled polygons use even-odd crossings at pixel centers, half-open y intervals and exact rational comparisons; 3–4096 vertices, no repeated terminal vertex. Self-intersections use even-odd semantics; outlines join the declared line rule.

Flood fill discovers an exact original-RGBA four-connected component before mutation, restricted by region/mask; iterative queue, no recursion. Pattern fill later tiles an immutable source modulo its dimensions with explicit phase. Copy/stamp reads an immutable pre-operation source even when source aliases any target. Copy includes zero-alpha samples and clears. Over uses the architecture's exact alpha rule. New cels are transparent; replacement must be explicit.

Transforms are exact pixel permutations/replication: quarter-turns clockwise; flips reverse the named axis; integer scale 1–16 replicates pixels; crop uses a valid half-open rectangle and adjusts cel offset. Grid reduction requires dimensions divisible by [gx,gy] and explicit phase inside the cell; it selects one sample per block, never guesses the phase. No transform invents art or silently updates pivot/anchors. Four quarter-turns or two identical mirrors restore samples and dimensions.

## Palette, cleanup, masks and controlled generation

Exact palette extraction counts visible RGB values from the selected cel, sorts by decreasing frequency then lexicographic RGB, and assigns names c000, c001, etc. Ignore fully transparent pixels; empty source is E_PALETTE_EMPTY. If distinct colors exceed max_colors (1–256), return E_PALETTE_TOO_MANY plus counts; do not silently truncate or quantize. Inspection histograms remain available when extraction cannot fit.

Recolor mappings are simultaneous against original samples, never cascading. RGB matching preserves alpha by default; explicit RGBA replacement is available with preserve_alpha=false. Palette changes affect future symbolic drawing and validation, not already drawn pixels. Quantization chooses nearest squared distance in encoded RGB, ties to earlier palette entry, ignores alpha-zero samples, and preserves alpha. Only explicit alpha cleanup changes alpha. Alpha threshold maps values below threshold to 0, others to 255; preserve RGB in either case. Color key matches exact RGB or RGBA as requested and sets alpha to zero without changing RGB. This makes hidden-sample changes deliberate.

A binary mask is ordinary RGBA image data restricted to opaque white/transparent black. Alpha predicate uses alpha>=threshold; exact-color predicate uses requested matching. Morphology later performs synchronous passes, radius 1–16, outside false, center+cardinals (4) or center+all neighbors (8). Outer outline=dilate(mask) minus mask, cropped to source dimensions; supply margins explicitly. Effects remain editable cels rather than non-destructive graph nodes. Shadows use recolored masks and explicit offsets; shading/ramp maps are explicit color decisions.

Core noise uses a seed encoded as exactly 16 hexadecimal digits, no OS randomness. With wrapping u64 arithmetic, mix(z): xor with z>>30, multiply 0xbf58476d1ce4e5b9; xor with z>>27, multiply 0x94d049bb133111eb; xor with z>>31. For local x,y, h=mix(seed+x*0x9e3779b97f4a7c15+y*0xbf58476d1ce4e5b9). Write when h%1000<density; choose colors[mix(h)%len]. Density 0–1000. Modulo bias is accepted; this is not cryptographic or a semantic texture model. Batch explicit clone/map/noise/stamp operations create controlled variations; a changed seed need not look distinct.

Advanced Bayer4, if admitted, uses rows `[0,8,2,10;12,4,14,6;3,11,1,9;15,7,13,5]`. Given nearest c0,c1, d=c1-c0, den=dot(d,d), num=clamp(dot(p-c0,d),0,den), select c1 if `32*num>(2*B+1)*den`, else c0. One-color palette always chooses it. Phase is cel-local plus offset and explicit phase, with Euclidean modulo. This retained algorithm is not part of the Core command enum until its schema and fixtures are added.

## Animation, registration and reuse

Frame cloning copies duration, anchors, metadata and image references; optional duration explicitly overrides. Edits detach shared pixels. Frame reordering does not rewrite named clip sequences. Missing/deleted references fail, never retarget by index. Pingpong expands n>1 as 0..n-1 then n-2..1, with endpoints not repeated; n=1 is [0], n=2 is [0,1]. Duration belongs to referenced frames. Repeat=0 is infinite playback metadata, not an execution loop; positive values mean total plays.

Named components remain ordinary assets/cels. Multi-target stamp supports a single source/placement across explicitly selected frames; different orientations/placements require explicit source artwork/operations. Replacing a component on four frames MUST leave body pixels, other frames, timings, clips, pivots and anchors unchanged. No naming convention implies automatic tracking, IK or pose inference.

Native transfer copies one complete asset and only referenced palettes/styles. Equal-name/equal-value dependencies reuse existing entries; unequal collisions fail E_CONFLICT. Retarget internal references; retain asset/frame metadata. No unrelated project state/maps/export requests and no live link to the source. Transfer does not automatically attach the asset to another.

## Export and inspection

Frame export is the Foundation preview mechanism; it emits an unscaled game image or an integer-scaled nearest-neighbor preview according to request. Contact sheet uses the same renderer. Onion composites the previous/current/next rendered frames with alpha multipliers 64/255/64 in that order; diff PNG marks every changed RGBA sample yellow opaque, unchanged transparent, with separate counts/bbox. Preview colors are excluded from underlying asset-palette checks. Cross-snapshot inspect comparisons report stored-cel or rendered-frame differences and must identify which was compared.

For n common-size frames w,h, columns c (1..n), padding p (0–64), extrusion e (0–16 and e<=p), scale s (1–16), rows=ceil(n/c): sheet size=`[c*(w+2*p),rows*(h+2*p)]*s`. Frame i rect starts at `[(i%c)*(w+2*p)+p,(i/c)*(h+2*p)+p]*s`, size `[w,h]*s`. Each cell owns its border; final unused cells are transparent. Extrusion fills an e-wide ring with clamped edge samples; remaining padding stays transparent. No trim, packing or rotation. Rects exclude extrusion. Clips reference exported indices, and missing selected frames fail. Distinct asset/frame pairs prevent identity collisions.

Sheet import uses this layout without extrusion: cell size equals asset canvas, explicit columns/padding/names/durations, exact source dimensions; unused final cells must be transparent. Existing layer required; new frames created by supplied names, duplicate existing frames fail. Alpha and hidden RGB samples are copied exactly. No trim/registration inference.

Objective checks include dimensions, allowed visible RGB, alpha policy, nonempty, bounds, structural references, positive timing, clips, required anchors and export layout. Registration compares explicitly requested anchor positions; equal anchors do not prove motion coherence. Inspect reports ordered names/state, alpha counts, distinct-color count, bounded histogram, bbox and bounded region pixels/symbols; checksums are non-cryptographic aids. A visual approval is a separate observation, never a validator score.

Advanced grid inspection should enumerate candidate integer cell sizes/phases and exact within-cell disagreement counts on a bounded selection. Report ambiguity, including uniform-region false positives; never auto-select a grid or claim irregular-grid reconstruction. Border-connected cleanup and tile seam analysis also need counterexamples before delivery. Edge equality is opt-in: a periodic checkerboard can be valid despite unequal opposite edge columns. Labels, numerical seam diagnostics and visual approval are separate checks.
