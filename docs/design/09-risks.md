# 9. Risk assessment

Likelihood/impact are qualitative judgments, not measured probabilities. A mitigation is a required future test or decision, not evidence that unimplemented software is correct.

| Risk | Likelihood / impact | Mitigation and gate |
|---|---|---|
| Independent PNG/DEFLATE implementation is incorrect | High / high | Foundation independent legal/malformed corpus, specification-clause review, output-length/window/tree/checksum bounds. No static-loop acceptance until real import works. |
| Zero-dependency effort is underestimated | High / high | Keep constraints, retire whole-suite schedule/LOC promises, measure S1 codec/parser work before planning Advanced scope. |
| Original artwork is technically valid but not useful/recognizable | Unknown / high | Scenario A from empty canvas with compact grids/high-level drawing; independent recognition/artistic review. A syntactically valid PNG alone fails product validation. |
| LLM commands remain verbose or fragile | Unknown / high | Per-op schemas/help, compact symbolic grids, target arrays, bounded region feedback, actionable pointer errors. Measure failures/tokens/correction loops across providers and compare actual baselines. |
| Flattened imported art lacks editable semantics | Certain for flat PNG / medium-high | Supplied masks, explicit named parts and component transfer; OpenRaster Advanced. No automatic anatomy/segmentation claim. |
| Hidden RGB, alpha or unselected pixels change | Medium / high | Preserve stored samples, explicit base-copy rendering, RGB maps preserve alpha, exact before/after diffs including transparent samples and all unrelated state. |
| Colors look different in managed consumers | Medium / medium-high | Explicit unmanaged encoded-RGB8 policy and profile warnings; sample-preserving is distinct from color-managed appearance. Do not claim complete PNG round trips. |
| Animation timing/registration/identity changes | Medium / high | Untrimmed canvases, named references, ms durations, explicit anchors, COW edits and independent consumer at scales1/2. New direction/pose artwork remains an artistic input. |
| Resource amplification/overflow | Medium / high | Checked wide arithmetic and limits on JSON, references, metadata, cloning, decoding and pixel visits; low-limit failure fixtures before allocation/traversal. Payload is not RSS; true OOM remains possible. |
| Native schemas grow or drift | Medium / high | Data-only JSON, current pixels rather than replay, required fields/versions, complete Core round-trip test and schema/examples checks. No automatic migration framework until needed. |
| New surfaces/manifest file count or stored PNG size become costly | Medium / medium | Measure realistic snapshot bytes/open/save time; shared allocations may serialize once. Add compression/archive only for a demonstrated bottleneck. |
| Layered formats lose semantics silently | Medium / high | Limited OpenRaster profile, unsupported features fail, layer order/opacity fixtures, correct required ZIP members. Do not label subset as full baseline or animation support. |
| Animated raster timing or disposal is misrepresented | Medium / high | APNG export only after exact duration mapping and independent playback tests; import Deferred until rational delays/disposal preservation contract. |
| Pixel-grid cleanup damages irregular artwork | Medium / medium-high | Explicit phase reduction Core; Advanced analysis reports candidates/ambiguity, no automatic correction. Irregular reconstruction needs evidence/algorithm review. |
| Tile diagnostics falsely claim seamlessness | Medium / medium | Separate labels, optional exact-strip rules, checkerboard counterexample, repeat/map preview and artistic review. |
| Game metadata is not directly usable | Medium / high | Explicit schema/coordinate units; independent consumer must prove slicing, timing, anchors and component placement. Prefer established profiles for specific future consumers. |
| Cross-platform paths/publication differ | Medium / high | Generated ASCII paths, Unicode validation, trusted parent, traversal/symlink checks, source immutable; test macOS/Linux/Windows from Foundation. No security-sandbox or power-loss durability guarantee. |
| Output transport fails after commit | Medium / medium | Rename boundary explicit, preserve destination, report committed when channel permits; caller checks before retry. |
| Independence is mistaken for differentiation | High / high | Six comparative tasks and real agent studies; no superiority/demand claim without evidence. Existing drawing/export capability is a parity requirement. |
| Extensibility becomes speculative framework work | Medium / medium | Modules/ordinary functions; no replacement traits/runtime plugins/optional dependency backends in the baseline. Revisit only for actual use. |
| Data/algorithm provenance is unclear | Medium / medium | Independently implement specifications; licensed fixture data with provenance, no copied/vendored code. Public documentation can inform behavior without importing implementations. |

Source snapshots are preserved and processing is local. Metadata cannot execute programs, direct output paths or trigger network reads. This does not imply the tool protects against a hostile concurrent filesystem owner. Artistic quality, coherent motion/perspective and useful variations remain empirical product risks, not solved by dimensions/palette/anchor checks.
