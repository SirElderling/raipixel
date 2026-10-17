# Scaled PNG previews: two complete alternatives (17 October 2026)

The Foundation CLI can export nearest-neighbor integer-scaled PNG frames
(scale 1–16, dimension/pixel ceilings). Exact original RGB, including RGB
beneath fully transparent pixels, must survive scaling. Compression settings,
work charges, deterministic output bytes and the PNG codec must not change.

## Implemented alternatives

- **A, previous two-buffer approach:** allocate a full scaled
  `Vec<[u8; 4]>`, push every output sample through the checked
  `Image::pixel(x / scale, y / scale)` access, then allocate and copy again
  using `Image::from_pixels`. Baseline preserved in
  `examples/bench_scale.rs`.
- **B, direct output image:** allocate only the final `Image` using the
  existing checked `Image::new`, fill horizontal repeat runs, then copy the
  completed row into its repeated rows using safe `slice::copy_within`.
  Production `Image::scaled_nearest` and the Foundation CLI use B.
  No unsafe code, FFI, assembly or production dependency.

Both do the same exact nearest-neighbor operation and charge the same work
(one unit per scaled output pixel, plus unchanged PNG writer work).
The benchmark checks full sample array equality, PNG byte equality, work
budget equality, and no change to original images. Unlike kernel-only trials,
the full-export timings include scaling, allocation, the **identical**
`png::encode_compressed` configuration and PNG buffer allocation; file I/O
is excluded.

## Measurements

Release builds, pinned Rust 1.99.1, seven alternating A/B and B/A rounds.
Absolute timings in microseconds. Each cell is **A / B**, median per complete
export, including PNG compression. PNG byte sizes are identical.

| Source → output | PNG bytes | Linux x86-64 | macOS ARM64 | Windows MSVC |
|---|---:|---:|---:|---:|
| 16² ×8 → 128² | 930 | 92 / 24 | 53 / 18 | 219 / 102 |
| 32² ×8 → 256² | 2,952 | 509 / 89 | 194 / 60 | 574 / 146 |
| 64² ×4 → 256² | 4,124 | 525 / 113 | 202 / 70 | 599 / 167 |
| 128² ×4 → 512² | 15,241 | 1,806 / 460 | 883 / 317 | 1,778 / 614 |
| 256² ×4 → 1024² | 59,843 | 5,786 / 1,973 | 3,619 / 1,243 | 7,227 / 2,417 |
| 128² ×4 (noise-like) | 70,125 | 1,860 / 1,147 | 1,439 / 909 | 2,753 / 1,491 |

[Native three-platform A/B run](https://github.com/SirElderling/raipixel/actions/runs/39068555161).

The separate scale-only microtiming also favors B on every measured
fixture. Whole-export speedups are about 1.6–5.7× for the complete corpus,
varying with compressibility, scale and host. These measurements do not
establish a universal speedup for unrelated encoders or image formats.

## Memory and decision

At 512², the previous path simultaneously owns two 1,048,576-byte scaled
RGBA buffers (2 MiB logical scaled-RGBA payload). The direct path owns one
1 MiB output. At 1024², the comparison is 8 MiB vs 4 MiB.
These are **logical buffer sizes, not measured process peak RSS**. PNG
encoder buffers, source image, operating-system allocator overhead and CLI
process memory are excluded. No process-wide RSS claim is made.

Choose **B** for both speed and predictably lower additional memory, with
a modest cohesive implementation and no new framework. `scale=1` continues
to pass the original source directly to the PNG encoder with no additional
image copy. The previous algorithm remains benchmark-only. Size validation,
zero/overflow handling, work rejection and exact transparent RGBA samples
are covered by `tests/scale.rs`, the unchanged PNG workflow tests and the
benchmark parity checks.

Remaining limitation: compare peak RSS and whole-process image throughput on
real user projects as part of S1/S5 gates. This does not establish general
compression-quality parity or complete S1 coverage.
