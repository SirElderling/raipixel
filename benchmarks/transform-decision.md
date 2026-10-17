# Exact whole-cel transforms: two distinct implementations (17 October 2026)

The Core increment implements `flip-x`, `flip-y` and `rotate-180`
for existing RGBA cels of fixed dimensions. More complex quarter-turn,
crop and scale transforms remain separate work. Input coordinates are
explicit, output is exact straight RGBA8, and hidden RGB must survive.

## Compared alternatives

- **A — in-place permutation, accepted**: `slice::reverse` on each row,
  row-pair element swapping, or `slice::reverse` of the entire raster.
  One work unit charged for each existing pixel **before mutation**.
  No additional image allocation; all three transforms are involutions.
- **B — copy-based coordinate mapping, rejected**: map every output coordinate
  back to its source, using checked pixel access and a pixel `Vec`, then
  build the destination `Image` via its public safe API. This approach uses
  two additional raster-sized buffers during construction (one staging
  vector, one output image). In production a different one-buffer copy-based
  variant might be faster than this reference; this comparison does not rule
  out every possible allocator or indexing strategy.

The comparison includes the **same input-image copy** before both algorithms
as a comparable complete independent task, and a separate complete PNG encode
at identical settings. Both outputs must have byte-for-byte identical pixels,
PNG bytes, and caller work accounting. The alternative uses a local image
creation budget in addition to its matching caller-visible work charge; it
is benchmark-only, not exposed as a resource-equivalent production path.

Seven alternating A/B and B/A rounds using Rust 1.99.1 release builds.
Values below are median **microseconds per operation** (A / B);
they include input copy and transform, with complete export columns additionally
including deterministic `png::encode_compressed`, not file I/O.
Fixtures use deterministic RGBA values, including zero-alpha RGB.

| Platform | 256² FlipX | 256² FlipY | 256² Rotate180 | 1024² FlipX | 1024² FlipY | 1024² Rotate180 |
|---|---:|---:|---:|---:|---:|---:|
| Linux x86-64 | 27.7 / 480.1 | 13.2 / 307.3 | 28.0 / 305.4 | 506.8 / 5118.9 | 269.0 / 5077.8 | 504.4 / 5320.1 |
| macOS ARM64 | 16.2 / 170.5 | 12.8 / 210.8 | 15.2 / 229.0 | 510.4 / 3825.4 | 413.5 / 3655.3 | 457.6 / 3651.3 |
| Windows MSVC | 76.9 / 590.8 | 40.7 / 537.9 | 53.7 / 435.4 | 2126.7 / 5562.9 | 1706.2 / 6217.7 | 1566.1 / 5616.1 |

Whole-operation speedups at 256² range from 7.7× to 23.3×.
Complete PNG export gains are smaller because compression dominates:
at 256², A/B times in milliseconds are 1.52/2.02, 1.50/1.80,
1.52/1.79 (Linux), 1.29/1.47, 1.39/1.60, 1.44/1.61
(macOS), and 2.00/2.37, 1.90/2.28, 1.92/2.31 (Windows)
for FlipX/FlipY/Rotate180 respectively.

PNG output sizes for those three inputs are 109,800, 109,755 and
110,097 bytes on all platforms and match exactly between A/B.
A uses no extra full-raster scratch. At 256², B uses 524,288 additional
logical bytes; at 1024², 8,388,608 bytes. These are **allocation
payload estimates, not measured peak RSS**. Full process memory and
allocation behavior on memory pressure remain unmeasured.

[Native release A/B workflow](https://github.com/SirElderling/raipixel/actions/runs/39071888888)
passed on Linux x86-64, macOS ARM64 and Windows MSVC.
[Native Rust CI](https://github.com/SirElderling/raipixel/actions/runs/39071892105)
also passed all three targets (fmt/tests/Clippy).
Reproduce with:

```sh
cargo run --release --locked --offline --example bench_transform
```

Select **A**: exact same output with lower measured runtime, no scratch,
bounded work before any mutation and a short, clear Rust implementation.
B stays only in the benchmark. The native snapshot semantics and PNG encoder
are unchanged. The executable rejects unimplemented transform kinds and
unknown fields instead of silently approximating them.
