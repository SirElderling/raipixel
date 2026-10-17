# Two-implementation measurement rule — PR #2

This rule applies throughout PR #2, including Foundation, Core and subsequent
specification work. It is a review/acceptance gate, not permission to introduce
dependencies, unsafe Rust, speculative frameworks or unbounded experiments.

For **each substantive implementation choice**, implement at least two
meaningfully different, viable solutions (the existing implementation may be
candidate A). Do not count cosmetic rewrites or differing compiler flags as two
algorithms. An implementation is not accepted as optimal until the alternatives
are compared. If two safe, specification-compliant alternatives cannot be
implemented, record the constraint and leave the choice provisional rather
than inventing a benchmark win.

For each comparison:

1. **Correctness first.** Assert exact RGBA values (including hidden RGB),
   state, output bytes when canonical, determinism, interoperability, errors,
   work limits and malformed-input behavior. Compare the same public task.
   A faster but behaviorally different method is not a valid replacement.
2. **Measure the whole operation.** Use release builds, the same compiler,
   inputs, public API and output quality/settings, and multiple alternating
   A/B and B/A rounds. Include initialization, allocations and validation
   when they are part of normal invocation. Check machine/compiler fingerprints.
3. **Record all dimensions.** Absolute timings and ratios by realistic small,
   common, large and adversarial inputs; output PNG size or image quality
   where applicable; heap allocations and **measured peak memory** where
   available, otherwise clearly labeled capacity estimates; scaling,
   maintenance complexity and failure modes. Never label a scratch estimate RSS.
4. **Use native measurements.** Compare Linux x86-64, macOS ARM64 and
   Windows MSVC when applicable. Avoid drawing conclusions from marginal,
   inconsistent or uncontrolled changes; repeat measurements or preserve the
   simpler, proven implementation until the evidence is strong.
5. **Choose the overall optimum.** Correctness/security/format fidelity are
   non-negotiable. Prefer compression quality for optional Maximum export
   within reasonable runtime/memory budgets, interactive latency for ordinary
   AI edits and predictable memory for hostile inputs. A clear speed win may
   justify small localized complexity; marginal wins must not add permanent
   complexity. Keep rejected candidates in reproducible experiments, not
   production.
6. **Record the decision before accepting.** Commit the candidate benchmark,
   its representative fixtures, native run links, absolute results/ratios,
   memory evidence, selected/rejected variant and limitations. Ensure all
   relevant tests and cross-platform CI pass and the old algorithm is removed
   from production (unless an explicitly justified mode needs both).

For changes involving straightforward wiring, formats or security boundaries,
the two implementations may be two materially different complete workflows
or publication/data structures; compare end-to-end performance and failure
semantics, not only a synthetic hot loop. If one is invalid, reject it on
correctness grounds rather than selecting it by raw speed. Benchmarks do not
override the specification.

## First application: exact palette extraction

The accepted implementation sorts a bounded scratch RGB array.
An experimental alternative counts exact colors in a fixed-capacity hash
table, falling back to the accepted implementation for pathological inputs.
Both must yield identical ordered RGB/count tuples; overflow must not silently
truncate or quantize colors. The experiment measures end-to-end extraction
and compares additional scratch, not process RSS. The hash variant remains
experimental until measurements and full resource-limit semantics justify
adoption. Run:

\`\`\`sh
cargo run --release --locked --offline --example bench_palette_extract
\`\`\`

Native Actions workflow: \`.github/workflows/palette-extract-compare.yml\`.
The existing \`examples/bench_color_map.rs\` provides another precedent:
a bounded hash lookup against sorted binary search on identical mappings.
