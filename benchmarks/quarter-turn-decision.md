# Exact quarter-turn rotation: A/B decision (17 October 2026)

## Contract

`Transform::Rotate90` rotates pixels clockwise; `Rotate270` counterclockwise.
For a source of width W and height H, the result is H × W, with all four
RGBA bytes, including RGB under alpha zero, preserved exactly.
The operation is deterministic and uses no interpolation. A failed work or
allocation preflight leaves the source image unchanged. The byte-exact PNG
output is independent of the chosen rotation algorithm.

The native `Image::transform` API handles rectangular images. The first
single-cel CLI/manifest profile currently accepts quarter-turn instructions
only for square canvases: rectangular rotations change cel dimensions, which
cannot yet be persisted separately from the canvas. It explicitly rejects
unsupported rectangular requests rather than corrupting a snapshot.

## Candidates

- **A (selected): destination-row gather.** Allocate one RGBA output buffer
  through `Image::new`, traverse output rows sequentially and gather each
  pixel from its mathematically mapped source coordinate. Charge output
  pixel count, then replace the image only after success. No extra scratch.
- **B (rejected): source-row scatter.** Traverse source rows and scatter each
  pixel into its mapped destination location. Requires the same output
  capacity, exact work and canonical PNG bytes but writes nonsequentially.

The reproducible `examples/bench_quarter_turn.rs` includes both independent
kernels, compares pixel buffers to each other **and** the production API,
then confirms byte-identical compressed PNGs. Seven alternating A/B and B/A
rounds are used. Each report shows both isolated kernel time and a full
operation including allocation, construction and `png::encode_compressed`,
excluding filesystem I/O. The full benchmark also reconstructs an `Image`
from the generated `Vec<Pixel>`, and therefore uses **one additional RGBA
allocation relative to the production A implementation**; absolute full
timings are not production CLI timings. This extra cost is identical for A/B
and is acknowledged, not hidden.

## Native measurements

Rust 1.99.1, release; median microseconds per invocation (convert 1,000 µs
to 1 ms). `A / B` represents destination gather / source scatter.
The benchmark prints every raw case, direction, output byte count and ratio.

| Image/direction | Linux full A / B (µs) | macOS ARM64 full A / B (µs) | Windows MSVC full A / B (µs) |
|---|---:|---:|---:|
| 16×16 CW | 2.938 / 2.929 | 4.324 / 4.297 | 20.332 / 21.020 |
| 64×64 CW | 20.586 / 20.614 | 33.294 / 33.096 | 75.320 / 74.915 |
| 256×256 CW | 763.645 / 893.825 | 683.283 / 693.558 | 1171.980 / 1290.500 |
| 256×256 CCW | 765.873 / 890.821 | 770.408 / 833.058 | 1181.700 / 1296.960 |
| 1024×256 CW | 3410.439 / 3647.090 | 3212.188 / 3018.959 | 4468.550 / 4458.750 |
| 1024×256 CCW | 3309.552 / 3676.133 | 3323.209 / 3215.479 | 4377.750 / 4629.950 |
| 1024×1024 CW | 13954.314 / 15456.806 | 11056.500 / 13537.000 | 16322.300 / 18921.500 |
| 1024×1024 CCW | 15806.542 / 15407.012 | 11515.459 / 15837.417 | 16184.700 / 21911.500 |

At 256² clockwise, A is 14.6% faster by B/A time ratio on Linux
(1.170× ratio), 1.5% on macOS (1.015×), and 10.1% on Windows (1.101×).
At 1024² clockwise, A wins on all three targets (B/A 1.108×,
1.224×, 1.159× respectively). On macOS 1024×256 clockwise, B is
approximately 6% faster (B/A 0.940×). On Linux 1024² CCW, B is
approximately 2.5% faster (B/A 0.975×). Small-image differences are
typically insignificant compared with complete PNG encoding.

Exact PNG sizes, in bytes, are unchanged for A/B:
16² CW 549, 64² CW 4299, 256² CW 60298,
1024×256 CW 241238, 1024² CW 743413.
Both candidates have one destination pixel buffer (`4 × W × H`
logical bytes). The benchmark's additional `Image::from_pixels` copy
is separate. **Peak RSS and allocator high-water marks were not measured**.

[All three native A/B jobs passed](https://github.com/SirElderling/raipixel/actions/runs/39074302720).

## Decision and limitations

Retain **A**, the simpler destination-order gather already used in
`src/transform.rs`. It has bounded space, predictable row-major writes
and the best general tradeoff without architecture- or shape-specific
dispatch. No reason to store B in production or add a new strategy mode.
The alternative remains solely in the reproducible benchmark.
This is a measured choice for the fixture suite, not a universal guarantee;
other CPU/cache layouts, images, toolchains and PNG profiles may differ.

Run `cargo +1.99.1 run --release --locked --offline --example bench_quarter_turn`.
Native CI also runs formatting, tests and Clippy. The PNG codec is unchanged.
