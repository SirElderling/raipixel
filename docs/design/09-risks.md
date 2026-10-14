# 9. Risk assessment

Qualitative likelihood/impact judgments, not measured failure probabilities. Release gates below are assigned to milestones; no unresolved risk silently changes the standard-library constraint.

| Risk | Likelihood / impact | Mitigation and release evidence |
|---|---|---|
| Handwritten PNG/DEFLATE bug | High / high | Independent supported/malformed corpus, exact expected sizes, checksum/order/tree/backreference validation, separate graphics review; M4/M6 block release on failures |
| Clone/recipe amplification | Medium / high | Global structural/string/reference/expanded-command caps, payload budget and nonpixel work charges; low-limit stress fixtures before any amplification allocations |
| Geometry overflow/excess work | Medium / high | i32 coordinates/bounded extents, checked preflight, wide arithmetic and charged traversal; far-off-canvas lines fail before looping |
| Incorrect alpha/color semantics | Medium / high | Straight-alpha golden vectors, byte-space policy, canonical zero alpha, palette/alpha validation of final render; no silent quantization |
| Registration/timing/identity loss | Medium / high | Fixed untrimmed canvases, asset pivot/anchors, explicit frame/clip identity and indices, component scaled/logical metadata; consumer and unaffected-state tests |
| Imported art lacks semantics | Certain for flattened PNG / medium | State limitation explicitly; masks/source components/native import enable targeted editing; no claim of automatic object extraction |
| Agent cannot create convincing art | Variable / high product impact | External source/artistic model/human review, representative visual gates; technical pass never labels art “good” |
| Direction or action inconsistency | Common in reviewed anecdotes, frequency unknown / high | Require supplied views/poses; one palette/anchors/constant components reduce avoidable changes; no unseen-view inference or fake rigging promises |
| Tile equality misclassified as seamlessness | Medium / medium | Labels structural; exact-strip condition opt-in; checkerboard counterexample; seam mosaic and external inspection required |
| Palette normalization harms artwork | Medium / medium | Nearest/Bayer algorithms documented; preserve alpha default, explicit binary threshold, histograms and editable original snapshot |
| Interoperability limits block sources | Medium / high | Clear8/sub-byte PNG support including Adam7; unsupported16-bit/APNG precise errors; record consumer needs before adding codecs; existing authoring tool is optional workflow choice |
| Stored-block exports too large | Medium / medium | Exact indexed PNG option; measure game collection sizes; don't claim compression equivalence; compression later only with demonstrated distribution need |
| Fixed grid too large | Medium / medium | Per-animation/per-category sheets or individual frames, allocation/work preflight; defer packing until actual texture-budget evidence |
| Native schema evolution/old recipes change | Medium / high | Snapshot stores pixels; versioned restricted grammar and engine semantics; explicit future migration, byte round trips; high-cost ADR change requires tests |
| Cross-project dependency name conflict | Medium / medium | Equal-value palette/style reuse, unequal conflict with actionable rename/rebuild hint; no lossy merge or implicit palette replacement |
| Platform path/CRT/output behavior | Medium / high | ASCII internal filenames, UTF-8 serialized paths, known symlink/path rejection, trusted output parent; clean macOS/Linux/Windows tests; Windows static CRT |
| Publication/transport confusion | Medium / medium | Rename is commit boundary; source immutable; post-commit broken-pipe results retained; agents check destination after transport error; no automatic overwrite |
| Power loss or hostile filesystem races | Low in intended owned workspace / high | Explicit lack of power-loss guarantee/hostile-directory sandbox; flush file writes, same-filesystem staging; use owned parent; no cross-platform atomic-replace claim |
| True allocator OOM/OS termination | Low within normal workload / high | Limits/try_reserve/detach accounting; metadata/string caps; failure fixtures lower budgets; do not promise every OOM is recoverable |
| Feature creep/underestimated code | High / medium-high | F01–F25 baseline, deferred admission rules, measured M1/M4 counts, preserve meaningful functionality; revise estimates openly rather than arbitrary line targets |
| Command drift or overly verbose recipes | Medium / medium | Shared typed descriptors/help, all ten recipes become integration tests, agent usability test; parameters/flat repeat solve batch overhead without language growth |
| Weak research conclusions | Medium / medium | Primary docs and conservative firsthand evidence grading, historical issues distinct from current defects, no invented consensus/benchmarks; validate tasks rather than market claims |
| Algorithm/license reuse | Low-medium / medium | Original std-only implementation from specs; check license/provenance before ports/fixtures; optional advanced algorithms deferred, no blind code copying |

## Security/correctness boundary

The CLI is a local tool processing explicit files, not a multitenant service. Safe Rust and bounds do not establish a filesystem sandbox or protect against malicious simultaneous changes to the output parent. Native manifests cannot execute recipe operations, import external files, traverse paths or run programs. Recipes can only invoke known deterministic operations and explicit input reads, with no shell expansion inside the engine. Output filenames are generated; metadata cannot redirect writes.

Pixel/profile bytes and artwork may be confidential. Processing is local, no logs/network by default; error output avoids arbitrary raw contents. Provenance strings are supplied by the caller; output may include them only when explicitly stored as metadata. Checksums are corruption/diagnostic aids, not authenticity/security hashes.

## Residual artistic limits

Technical consistency means exact dimensions/palette/alpha/anchors and metadata. It does not guarantee coherent perspective, attractive shading, anatomy, action readability, visual seam invisibility or a game's art direction. No deterministic transform can reconstruct occluded anatomy or invent action poses. The suite remains useful because it preserves editable parts, reduces repetitive correction and produces precise game-consumable output; artistic sources and approval remain outside it.
