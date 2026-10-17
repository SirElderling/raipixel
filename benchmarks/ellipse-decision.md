# Exact ellipse rasterization — measured two-method decision (17 October 2026)

## Requirement and invariants

`draw` with `shape=ellipse`, `rect=[x,y,width,height]` and
`filled=true|false` selects cell centres using the integer conic inequality
from `docs/design/05-functional-specification.md`. An outline consists of
inside cells with at least one cardinally adjacent outside cell. Clipping must
not manufacture geometry-boundary pixels. Bounds and complete declared work
are validated before writes. All original samples outside the shape, including
RGB where alpha is zero, are preserved. No floating point, unsafe, assembly,
production dependencies or separate image buffer.

## Two viable implementations

- **A — pixel-centre scan:** test every pixel against the full i128 ellipse
  inequality, then test four neighbours for an outline. Straightforward,
  predictable and low overhead for tiny fixtures, but expensive conic
  multiplication for every candidate pixel.
- **B — row-span search, accepted:** for each relevant row, binary-search
  the leftmost inside centre and mirror it to obtain the rightmost. Filled
  rows are written as one contiguous RGBA slice. Outlines compare each row's
  span with the two adjacent geometry rows. The costlier conic predicate is
  evaluated O(height × log(width)) rather than once per candidate. Both
  retain O(1) auxiliary scratch and exact original-sample behavior.

The first measured A/B benchmark used the production scan in A and a
**public-API `Image::fill` row-span prototype** in B, including repeated
per-span validations. B was conservative relative to the subsequently
accepted direct-row-slice implementation. It measured a full input copy and
shape edit, then separately input copy, edit and `png::encode_compressed`.
Both used equivalent caller-visible charged work. Neither benchmark times
disk writes, project-file publication nor measured peak RSS.

### Native results

Rust 1.99.1, release, seven alternating A/B and B/A rounds, medians.
The values in each pair are A / B. Full image dimensions also equal ellipse
rect bounds; the source has deterministic varying RGBA with hidden RGB.

| Platform | 256² filled edit (µs) | 1024² outline edit (ms) | 1024² outline plus PNG (ms) |
|---|---:|---:|---:|
| Linux x86-64 | 205.36 / 46.78 | 8.64 / 1.20 | 10.91 / 3.46 |
| macOS ARM64 | 117.92 / 37.77 | 4.65 / 0.57 | 6.42 / 2.53 |
| Windows MSVC | 183.52 / 74.68 | 7.10 / 2.13 | 9.86 / 5.12 |

The row-span method was **2.46–4.39× faster** for the 256² filled
edit and **3.34–8.21× faster** for the 1024² outline.
At 16² filled the scan was 0.27–0.92 µs faster than the row-span
prototype, an absolute difference not worth per-size dispatch and
maintaining two production kernels. On 32×64 filled, both were effectively
equal (about 1–7% apart). The selected implementation keeps one method
with no arbitrary size threshold.

Both A and B produced identical RGBA samples and complete PNG bytes for
every measured filled/outlined fixture. The 1024² outline PNG was
**160,659 bytes** with both; the 1024² filled PNG was **88,622 bytes**
with both. Work charges matched. The path uses no raster-sized scratch;
heap capacity, allocator retention and peak process RSS were **not measured**.
These are finite fixture results, not a universal speedup claim.

[Three-platform A/B native run](https://github.com/SirElderling/raipixel/actions/runs/39075770722).
The exact pre-promotion comparison source is retained in commit
[`ac21f6f`](https://github.com/SirElderling/raipixel/commit/ac21f6f377a0b3a9b5bb6f0c8dc0ad7d1dcdcdf5).
The working `examples/bench_ellipse.rs` retains independent
scan and row-span algorithms alongside an assertion that the promoted
production path equals both; run
`cargo run --release --locked --offline --example bench_ellipse`.

## Decision

**Accept B** for both filled and outlined ellipses. The larger-fixture
speedups and lower full-PNG export latency are material and consistently
reproduced on three native architectures. The implementation adds a
short local binary search and two adjacent-row lookups without heap scratch
or API abstractions. Keep one kernel to honor KISS; retain the rejected
scalar method only in the benchmark and historical commit.

The next project-level dependency remains independently sized layered
cels and complete editable snapshot/render round trips. Adding more
standalone raster effects is not a substitute for completing S1.
