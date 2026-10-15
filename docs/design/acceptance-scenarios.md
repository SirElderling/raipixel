# Acceptance scenarios and comparative validation

All gates are prospective; no application or comparative test has run. A–E are required for the first useful Foundation+Core toolkit. S1's static subset is independently useful but does not satisfy the entire product gate. Use [valid proposed workflows](examples/README.md) as starting fixtures; they are not executed evidence.

## First useful toolkit: mandatory scenarios

| Gate | Workflow | Technical pass | Separate visual/product evaluation |
|---|---|---|---|
| A — Original artwork | An LLM starts empty, creates a recognizable asset with grids/palette-aware drawing/components, renders/inspects it, corrects a chosen detail and exports PNG | No source art in the creation step; valid owner/operation sequence; save/reopen preserves cels/palette/pivot; deliberate revision changes intended samples; independent PNG decode matches expected pixels. Examples1/2. | Reviewers receive the image without the intended label and identify its subject; record recognition and artistic-quality judgments separately. A valid colored rectangle does not prove useful original creation. Include a less trivial held-out icon/prop after the sword fixture. |
| B — Existing artwork | Import a real independently produced PNG, inspect it, correct one known defect and export | Exact RGBA comparison before/after: only declared selection changes, including zero-alpha RGB outside it; dimensions/alpha/timing/metadata preserved as promised; no source overwrite. Exercise compressed/interlaced/palette-alpha inputs, not only the native writer. Example3. | Confirm correction actually solves the selected visual defect; unmanaged color-profile limitations are reported, not hidden. |
| C — Animation/component correction | Author or assemble at least four frames with nonuniform timing, clips and anchors; modify one named equipment/armor component across selected frames, export frames/sheet+metadata | Frame count/order, durations, playback/repeat, pivot/anchors and unchanged components match expected state. Consumer places frames/components correctly at scales1/2. Edit one cloned frame to prove COW independence. Two-frame examples6/7 are smoke workflows; extend to four for this gate. | Review motion separately. New poses/views must be explicitly authored or supplied. Static translations alone cannot establish credible character animation. |
| D — Reproducibility | Replay the same documented JSON job against the same input into fresh destinations | Equal complete semantic state and decoded pixels; same engine/writer version/settings yields identical canonical snapshot PNG/JSON and exported bytes across repeat runs and supported targets. Compare bytes, not only CRCs; absolute report destinations excluded. | Record whether chosen deterministic variations are useful/distinct; reproducible noise alone does not prove quality. |
| E — Interoperability | Open exported PNG and game metadata with independent consumers; reopen/resave native project; exercise later format conversions only when delivered | E1: PNG samples exact in supported profile. E2: native equality includes layers, frames, timing, pivots/anchors, palettes/styles, hidden samples, clips, metadata and references. E3 (Advanced): supported OpenRaster layers round-trip, unsupported rendering features fail. E4 (Advanced): APNG rendered sequence/timing/loop independently verified. | Check actual engine slicing/registration/playback, not only syntactic JSON. Fidelity reports identify omitted/converted semantics; never claim complete editor/animation round trips through flat raster formats. |

An independent PNG decoder/viewer and a small game-consumer scene are verification tools, not runtime dependencies. Record exact consumer identity/version/settings in test results. A generic JSON parser is not proof of game compatibility. For OpenRaster compare stored layer samples and order/offset/visibility/opacity separately from rendered output; clipping/rounding/compositing differences require explicit diagnosis. APNG cannot preserve named cels/anchors; do not advertise it as editable project interchange.

## Comparative task protocol

Compare equivalent end results against representative sprite-editor automation, procedural raster automation, and an AI-native generation/editing workflow where applicable. Baseline identities are recorded in the actual experiment report, outside these product-neutral design documents. Do not invent a command or absent capability for a baseline: verify its current manual, pin version/settings, record exact commands and document any manual bridge. No controlled results or superiority are claimed here.

| Task | Shared fixture and goal | Objective outcomes |
|---|---|---|
| 1 Original creation | Same text brief, empty canvas, dimensions and palette; small prop/icon plus held-out character/scene detail | Recognition/artistic rating separate; editable structure, validity, prompt/tool tokens, calls and correction iterations |
| 2 Import correction | Same real PNG and known defective region; preserve outside | Exact changed-sample set including transparent samples, preservation of promised state, independent decode and visual fix |
| 3 Multi-frame component | Same layered four-frame fixture/timing/anchors; replace one named component on selected frames | Component changed as specified, unaffected layers/frames/timing/anchors exactly equal; usable output |
| 4 Consistent variants | Same base and at least20 explicit variations, palette/shape/seed constraints | Requested count, controlled differences, unaffected-state checks, replay equality; usefulness/diversity judged separately |
| 5 Game export | Same animations/registration, padding/extrusion/scale; import into an independent consumer | Rect geometry, timing, clip identity, pivot/anchors, component offsets, actual playback and declared conversion losses |
| 6 Agent efficiency/recovery | Run tasks1–5 with multiple providers/model versions and seeded invalid/misspelled/reference/bounds requests | Command failure rate, actionable recovery success, input/output/tool-result tokens, process/tool calls, end-to-end time, retries and correction iterations |

Use identical source art and output constraints for deterministic edit tasks. Generation comparisons must record differing source/model access rather than assume equivalent creative inputs. Separate host overhead, engine runtime, model latency and human interventions. Tokenize with each model's actual tokenizer and include discovery/errors/inspection payloads; raw character count is not a token result. Record first-attempt and recovered success, correctness/quality, cost and failures; never compare token counts while ignoring failed outputs.

Repeat the fixed tasks across more than one model/provider and multiple trials; record sample counts and variation/ranges, not only the best run. Define the stopping budget and correctness criteria before the experiment. Keep all generated artifacts, exact prompts/jobs/commands, fixture provenance/digests, versions and judgments sufficient for replay. The success hypothesis is lower effort, fewer errors, better reproducibility or more reliable edits for a useful task, not automatic victory on every metric.

## Later demanding scenarios

| Scenario | Required evidence before acceptance |
|---|---|
| Character/equipment collection | Eight explicitly authored/supplied directions, idle/walk/action, per-frame equipment/occlusion, palette and fixed footline. Verify unchanged body/registration; evaluate anatomy and action readability externally. |
| Tile/environment production | Authored rectangular terrain/transitions, map import/export via established profile, collision/walkable metadata, repeat mosaics, label mismatch fixture and checkerboard counterexample to edge equality. No full terrain/vision solver implied. |
| Collection-wide consistency | At least48 mixed assets, shared technical styles, independent component transfer, palette conflicts and batch corrections/exports. No subjective style guarantee. |
| Layered interchange | Independent OpenRaster sources with offsets/visibility/opacity, required archive members, unsupported blend/group rejection, exact supported-layer round trips; no animation assumed. |
| Grid/transparency correction | Explicit-grid perfect input, ambiguous uniform input, irregular-grid counterexample, border key matching an internal detail. Show deterministic diagnostics and user-chosen repair without semantic overclaim. |

Every gate also runs invalid variants: missing/duplicate refs, malformed PNG/JSON, over-budget metadata/clone/decode, out-of-bounds writes, invalid palette/clip, output collision and failed staging. Assert stable code, pointer/target and repair hint, no source modification or pre-commit output. Test broken reporting after rename separately: committed output remains.

A gate report records fixture IDs/provenance, target/compiler/profile, selected capability set, technical results, rendered artifacts, independent consumer outcome, visual judgment, measured time/memory/tokens and unresolved limitations. No planning estimate substitutes for an execution report.
