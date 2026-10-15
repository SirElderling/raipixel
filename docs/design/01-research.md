# 1. Capability research and user needs

Research cutoff: **2026-10-14**. This is a scoped qualitative assessment, not an exhaustive market survey, independent benchmark, or statistical estimate of demand. Product names, project identities, version numbers, vendor links and named comparisons are intentionally omitted. The original review considered technical documentation and firsthand reports, but individual observations cannot be independently verified from these source-anonymized documents. They inform requirements and tests; they do not establish defects in any current external product.

## Capability categories assessed

| Category | Capability patterns considered | Design implication |
|---|---|---|
| Desktop raster editing | Layered cels, palette controls, sheets, frame timing, batch export, editable native projects | Preserve editable structure, exact timing and reproducible exports rather than cloning an interactive UI |
| Browser-based sprite editing | Frame/palette workflows, compact drawing tools and image exports | Include fundamental pixel operations and open interchange; require no browser at runtime |
| Procedural graphics | Seeded variations, composable effects and reusable assets | Supply bounded deterministic operations and batch recipes without a node graph or scripting runtime |
| Hosted image generation | Prompt/reference-guided art, directional outputs and repair masks with varying provider behavior | Consume externally supplied images without a model client or artistic-consistency promise |
| Narrow local generators | Efficient production for limited sizes or formats, often without broad edit/animation support | Prioritize correction, reuse, animation and export over a single specialized generation task |

These categories describe capabilities considered in the assessment, not capabilities guaranteed by every tool. No product ranking, version comparison, benchmark or feature-absence claim is made.

## Qualitative user-feedback ledger (source-anonymized)

IDs U1–U17 preserve cross-document decision references. Names, source URLs, issue identifiers and publication details are omitted by design. Qualitative strength reflects how concrete the originally reviewed description was, not how prevalent its problem is. Without source links these reports cannot be independently rechecked here; require acceptance tests before making implementation or marketing claims.

| ID and report category | Observed workflow or problem | Assessment and raipixel implication |
|---|---|---|
| U1 — public issue report | User repeatedly re-enters sheet settings and needs reproducible shared exports. | Moderate firsthand workflow. Saved export presets; do not infer a current defect. |
| U2 — public issue report | Local/global palette ownership and updates confuse user. | Moderate anecdote. Explicit project ownership and palette import/update semantics. |
| U3 — public issue report | Separate head/body linked cels export bad registration and render incorrectly in game. | Strong concrete historical workflow. Preserve canvas/offset/pivot identity. |
| U4 — public issue report | Multi-file sheet tags are ambiguous/misaligned; maintainer discusses interactions. | Strong workflow. Qualified asset/frame IDs and explicit clip indices. |
| U5 — public issue report | User needs edge extrusion to mitigate texture bleed without another preprocessing step. | Moderate concrete request. Low-cost extrusion distinct from padding. |
| U6 — public issue report | Padding/packing can leave sprites touching. | Strong correctness fixture. Fixed-grid non-overlap/layout tests. |
| U7 — public issue report | GIF timing rounding/clamping changes playback. | Strong repeated report/maintainer analysis. Milliseconds authoritative; defer codec-specific previews. |
| U8 — public workflow discussion | Bulk particles/foliage with varying seeds from shell; maintainer says CLI demand recurs. | Moderate-to-strong qualitative demand. Parameter batches, no need for node language. |
| U9 — public issue report | Near-duplicate colors disrupt palette/fill workflow. | Weak-to-moderate unreproduced report. Histograms and exact membership. |
| U10 — public issue report | Partial-alpha import/export changes; maintainer confirms then-binary pixel alpha. | Strong historical behavior, current unverified. Preserve RGBA; explicit thresholding. |
| U11 — first-person workflow discussion | Solo developer reports inconsistent attacks/frames/sheets and cannot scale cleanup to dozens. | Firsthand anecdote. Correction/reuse/batches; no vendor quality ranking. |
| U12 — first-person workflow discussion | Pose/proportion/background changes jump in motion; one animation abandoned. | Independent recurrence with U11, promotional context. Registration/alpha checks plus visual review. |
| U13 — first-person workflow discussion | Changing axe corrected by drawing one component and placing it with hand occlusion; palette/footline cleanup scripted. | Specific useful self-report, commercial promotion and unverified code. Named components/masks/anchors. |
| U14 — first-person workflow discussion | Regeneration changes eyes/shading; one base body and palette swaps improve reuse. | Independent novice anecdote. Controlled variation, not whole-asset regeneration. |
| U15 — first-person workflow discussion | Generator→grid correction→editor cleanup; participants ask palette controls and acknowledge irregular-grid failure. | Multiple participants, creator interests. Supplied grid reduction; no reliable automatic-grid promise. |
| U16 — first-person workflow discussion | Uses key-color backgrounds for cutting generated sheets. | Single report. Exact color key/masks, no semantic background inference. |
| U17 — first-person workflow discussion | Tile failures claimed; participants disagree and recommend specialized generators. | Contested anecdote. Tile inspection useful; not proof all AI tile output fails. |

The reports suggest practical issues around frame registration, palette/alpha control, component reuse and repeated export. They do not establish industry consensus, demand percentages, generation success rates, benchmarking results or current defects in any competing product. Technical validation cannot guarantee artistic consistency.

## Candidate-capability decisions

Every item in brief A–J is evaluated below; grouped items share the same decision and rationale. E essential, H high-value selected, O optional/deferred, N unnecessary/excluded now. Classifications are architectural judgments informed by sources, not measured preference rankings.

| Candidate | Class / smallest useful implementation | Evidence and complexity/reuse judgment |
|---|---|---|
| Pixels, lines, rect/ellipse/polygon, flood fill, copy/crop/translate/mirror | E; integer raster coverage and shared write traversal | Basic raster-editing requirement; creation/correction requires these. Low/medium shared cost. |
| RGBA vs indexed internal storage | E RGBA; O second indexed engine | U10 alpha; RGBA simplifies composition, RGB palette constraints retain style. |
| Masks/region selections/pattern fill/symmetry | H; binary cel masks, rectangles, repeated stamps/flips | U13 targeted occlusion; shared operations, no selection object system. |
| Alpha blending, integer scaling and quarter turns | E; defined source-over/exact permutations | U10/U15; preserves pixels and avoids new color noise. |
| Arbitrary-angle rotation and advanced pixel-preserving interpolation | O; defer | U13 demonstrates held-tool angle demand; v1 uses supplied angled poses. Quality heuristics/regression cost defer arbitrary-angle algorithms. |
| Named layers/order/visibility/opacity/duplication | E; flat layers plus cel/frame copy | U3/U13; targeted editing essential. |
| Layer groups/live clipping/non-destructive stacks | O; masks/recipes/separate effect cels instead | Useful to artists; weaker need for headless baseline, persistence/graph cost. |
| Reusable components/named refs/transforms | E; ordinary asset/cel references and native asset import | U13/U14; no rig hierarchy. |
| Custom/restricted palettes/import/export/replacement/remap/variants | E; ordered RGB palettes and exact maps | U2/U9/U13–15; high value per line. |
| Ramps/dither/controlled shading | H; explicit ramp map + Bayer4 | Palette reuse; user-priority evidence weaker, small shared cost. |
| Outline/silhouette/edge/morphology/shadow | H; alpha masks + dilation/erosion/subtraction/offset | Agent correction/effects, common editor capability; objective operations, no clean-outline art judgment. |
| Auto lighting/gradient-map infrastructure/advanced texture model | O; palette map + sparse seeded texture instead | No baseline demand sufficient for more systems. |
| Frames/cels/durations/clips/directions/alignment/metadata | E | U3/U4/U7/U11–13; first-class production behavior. |
| Clone/edit across frames/component animation | E; explicit copies/placements plus supplied poses | U13 remedy; rigid motion cannot replace new artwork. |
| Neighbor composites/contact/diff previews | H | U12/U13 diagnosis; keeps agent value of onion skin without UI. |
| General skeletons/IK/inferred poses/interpolation | O for rigging; N for artistic inference promise | High code/art assumptions; supplied artwork remains required. |
| Seeds/templates/variants/recombination/bounded batches | H; scalar parameters + flat repeat | U8/U11/U13/U14; no general language needed. |
| Rectangular tiles/maps/variants/transitions/edge labels/seam checks/collision | H; frames + explicit map and authored borders | Common editor capability/U17 contested; reuse renders, objective boundaries. |
| Autotile/Wang solver/procedural terrain synthesis | O; authored edge-labeled sets first | Generator features do not establish deterministic solver baseline demand. |
| Isometric/hex/navigation meshes | O; rectangle maps and walkable flags | Topology/engine-specific complexity; basic collision metadata selected. |
| PNG import/export/sheet slicing/frames/metadata/native projects | E | U1/U3/U4/U10/U13–15; highest necessary codec cost. |
| GIF/APNG/ASE/PSD/ORA/JPEG/WebP/16-bit PNG | O | Interchange useful, exact RGB/alpha/timing baseline through PNG; costly codec maintenance. |
| Saved export presets/padding/edge extrusion | H, selected | U1/U5/U6 direct pipeline needs, inexpensive shared copies. |
| Packed/trimmed atlases | O; fixed grids first | U6 correctness complexity; no baseline texture budget shown. |
| Dimensions/palette/alpha/bounds/frame/sheet/edge/metadata/reproducibility checks | E | U3/U4/U6/U9–13; technical facts only. |
| Automatic visual quality/style/anatomy judgment | N | Not supported by deterministic engine or evidence. |
| GUI timeline/canvas/audio/hotkeys/themes/pressure tools | N | Human interaction value, no headless benefit. |
| Runtime plugins/network/MCP/model integration | N | Violates or adds needless scope; ordinary modules + external agent sufficient. |

## Algorithm and standards references

The [W3C PNG specification](https://www.w3.org/TR/png-3/), [RFC 1950 (zlib)](https://www.rfc-editor.org/rfc/rfc1950) and [RFC 1951 (DEFLATE)](https://www.rfc-editor.org/rfc/rfc1951) establish required format behavior. [Rust build profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) document compilation controls, not size forecasts. Additional edge-reconstruction or interpolation approaches require explicit deterministic semantics, licensing review and representative tests before adoption.

## Opportunities and evidence gaps

The useful opportunity is a durable local repair/reuse/registration/export layer, plus deterministic creation primitives. A creation-only utility misses existing-asset correction; a model clone would violate constraints. Presets, native component transfer and precise JSON identity remove repeated integration work at modest cost.

Evidence gaps remain: no usability study with agents; no executed performance/format matrix; no executed comparison of external tools; no exhaustive native-format compatibility; no automatic semantic decomposition; no statistically ranked feature demand. The release plan therefore validates actual representative workflows and measures code/binary/runtime before expanding optional scope.
