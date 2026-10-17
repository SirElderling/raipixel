# Exact RGBA revision comparison A/B (17 October 2026)

The new `inspect --query` `against` operation compares original RGBA byte samples
from independently saved projects; hidden RGB is significant. It reports
changed samples, visible color/visibility changes, alpha changes, an exact
image-local changed bbox and non-pixel manifest equality. The candidate
comparison was tested before selection on the same `Image` inputs.

Two implementations:

- **A, full scan:** iterate each selected pixel and classify exact changed RGBA.
  One work unit per sampled pixel; no temporary heap.
- **B, row skipping:** first compare whole row slices for byte equality,
  classify pixels only on rows that differ. Identical results, charges,
  errors and bounds; still no temporary heap.

Both are safe stable Rust and use the existing single-pass bounded
`Work` policy. A remains as a frozen reference in
`examples/bench_image_compare.rs`; B is now the library production method.
The benchmark verifies exact result and charged-work equality before timing,
with further adversarial cases including hidden RGB, every-pixel differences
and independently selected regions.

## Native measurements

Seven alternating release rounds; median time per operation, in microseconds.
Rust 1.99.1. Each cell is **A / B**.

| Fixture | Linux x86-64 | macOS ARM64 | Windows MSVC |
|---|---:|---:|---:|
| 16² identical | 0.202 / 0.107 | 0.119 / 0.077 | 0.355 / 0.244 |
| 64² sparse changes | 3.069 / 0.545 | 1.583 / 0.559 | 3.231 / 0.899 |
| 64² dense changes | 8.592 / 10.779 | 5.818 / 5.758 | 8.559 / 10.246 |
| 1024² identical | 684.878 / 170.411 | 460.075 / 206.012 | 676.780 / 197.380 |
| 1024² sparse changes | 687.165 / 231.090 | 460.004 / 225.216 | 678.630 / 271.950 |
| 1024² dense changes | 2064.442 / 2623.909 | 1321.208 / 1415.795 | 2040.500 / 2367.440 |
| 1024² hidden-RGB changes | 681.731 / 699.276 | 463.654 / 531.820 | 675.840 / 724.800 |

[Native A/B CI, exact parity on all targets](https://github.com/SirElderling/raipixel/actions/runs/39067847207).

## Selection and limitations

Select **B, equal-row skipping**. Identical/sparse images gain about
2–5.6× over A across the tested large cases, with minimal implementation
complexity (one equality test per row). Dense changes regress by about
7–27% in the 1024² case, and hidden-RGB changes may regress by about
3–15%. For AI correction and iterative inspections, unchanged and
sparse edits matter; this is a measured overall trade-off, not universal
speed superiority. A second run after promotion must confirm parity
against the frozen full-scan baseline.

Both candidates allocate **zero additional heap buffers** for comparing already
decoded images (logical scratch only; no peak RSS sampled). These figures
measure the comparison operation, including bounds preflight and work charge,
not PNG decoding, process launch, JSON parsing or snapshot I/O. An end-to-end
CLI speed advantage has not been established. The full CLI comparison also
loads the other snapshot under the existing resource limits, and rejects
paths escaping the query directory. The snapshot-only `unrelated_state_equal`
flag compares canonical current schema fields and does not include artwork
bytes; pixel changes are covered by the other fields.

Neither candidate modifies images, PNG codec output or persistent image
semantics. Both honor exact four-byte RGBA equality.
