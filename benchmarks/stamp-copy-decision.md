# Same-cel Copy: snapshot versus overlap-safe row movement

Benchmark date: 17 October 2026. Exact safe-Rust, dependency-free raster
implementation, no FFI, assembly or unsafe code.

## Actual old/new alternatives

- **A (previous implementation)**: before writing, reserve and copy all pixels
  from the source rectangle into a separate contiguous `Vec<Pixel>`, then
  blend from that immutable snapshot into the destination, regardless of mode.
  This is the exact historical implementation at
  [9ee5a3b](https://github.com/SirElderling/raipixel/commit/9ee5a3b3977f4be50afb5fc2023a475318564a9f).
- **B (selected)**: for `Blend::Copy` at full opacity (255), process visible
  destination rows in top-to-bottom or bottom-to-top order depending on
  vertical movement; use the standard library's `slice::copy_within` for
  overlap-safe movement within each row. Avoid a separate source allocation.
  Other modes and partial-opacity Copy retain A's frozen source path, preserving
  blending and alpha semantics.

Both call the same public `stamp_within` API, return identical clipped counts,
charge **the same preexisting work units**, and preflight invalid geometry before
mutation. B preserves exact RGB under zero-alpha pixels. The source area is
still checked against the historical per-image byte limit; no limit has been
silently increased. This comparison preserves the engine semantics version.

## Native A/B evidence

The benchmark temporarily checks out A in a separate Git worktree, copies the
**same** public Rust benchmark driver there, compiles both under pinned
Rust 1.99.1 `--release --locked --offline`, and runs five rounds in alternating
ABBA/BAAB order. Each invocation includes the input-image copy and the edit;
the export variant also includes `png::encode_compressed` with identical
settings, excluding file I/O. Result pixels, full PNG files (byte-for-byte),
charged work and clip counts are compared before timing.

Edit times in microseconds, **A → B**, median per edit:

| Test | Linux x86-64 | macOS ARM64 | Windows MSVC |
|---|---:|---:|---:|
| 16² icon | 0.235 → 0.098 | 0.264 → 0.130 | 0.451 → 0.215 |
| 64² sprite | 7.015 → 5.645 | 0.916 → 0.435 | 11.962 → 7.979 |
| 256² sheet | 184.905 → 22.953 | 20.981 → 14.773 | 190.200 → 108.894 |
| 1024² large | 1,504.638 → 947.385 | 1,279.114 → 979.219 | 1,516.575 → 895.750 |
| 256² clipped | 32.355 → 21.138 | 18.721 → 14.591 | 82.850 → 18.662 |
| 256² upward overlap | 187.557 → 25.211 | 23.792 → 14.526 | 105.513 → 20.581 |

All 18 platform/fixture edit comparisons favor B. Measured edit speedups
range about 1.3–8.1×. PNG sizes are unchanged: icon 574 B, sprite 7,983 B,
sheet 79,511 B, large 1,435,062 B, clipped 76,870 B and upward 79,669 B.

Complete export is dominated by PNG compression. The 256² sheet export was
1.288 → 1.226 ms (Linux), 0.948 → 0.957 ms (macOS), and
1.183 → 1.097 ms (Windows). The macOS sheet and upward export cases
regressed by ~0.95% and ~2.34% respectively. These small changes should
not be treated as reproducible universal effects; the substantial memory
improvement and consistent edit wins justify B.

A's source rectangle snapshot costs `4 * width * height` additional
logical bytes: 200,704 bytes for the 224² sample and 4,000,000 bytes for
the 1000² sample. B uses no corresponding heap allocation for the
full-opacity Copy case. These are **logical temporary allocation estimates,
not measured peak process RSS**. All mode/opacity combinations outside this
specific Copy specialization keep the existing snapshot allocation.

[Native A/B workflow](https://github.com/SirElderling/raipixel/actions/runs/39072566710)
completed successfully on Linux, macOS and Windows, with raw samples uploaded
as CI artifacts. [Baseline after correctness/format integration](https://github.com/SirElderling/raipixel/actions/runs/39072354860)
passed the native Rust test suite on all three platforms.
The exhaustive small-canvas oracle test compares B with an immutable
original source for all valid source rectangles and overlapping/clipped
destination positions; separate tests cover work/bounds rollback and partial
alpha behavior.

Reproduce:
```sh
python tools/bench_stamp_compare.py --output stamp-compare.json
```

Select **B for full-opacity Copy**, retain A for other modes. Avoid generic
backend interfaces: this is a localized safe-Rust optimization.
