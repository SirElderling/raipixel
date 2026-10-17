# Exact nearest-palette quantization: native A/B/C decision (17 October 2026)

## Contract

`palette.quantize` operates on one editable cel and an existing named palette
of 1–256 ordered RGB colors, with `dither="none"`. For each visible pixel,
minimize the sum of squared encoded RGB8 channel distances; on ties, choose
the **earlier original palette entry**. Preserve alpha exactly. Pixels with
alpha zero retain **all four** RGBA bytes, including hidden RGB. Never change
pixels outside the selected region. Validate dimensions, references and work
budget before mutation. No floating point, source-dependent semantics or
external Rust crates.

The nominal work charge is selected pixel count times palette length,
regardless of the selected algorithm. A failed work charge leaves pixels and
the budget unchanged; malformed palette/region inputs fail before mutation.
All three algorithms reproduce the same exact pixel and PNG output on the
measured workloads. This is a constrained Core feature, not a claim that the
full layered/animated tool is complete.

## Alternatives

- **A: direct search.** One squared RGB distance per palette entry for each
  visible pixel; O(pixels × colors), no lookup scratch. This is the simple,
  bounded fallback, and the only path for small edits or varied RGB samples.
- **B: bounded repeated-RGB cache.** Fixed 4,096-slot array of
  `Option<([u8;3],u8)>`, 8-probe open addressing, exact search on cache misses
  and collision saturation. Same output and worst-case limit as A, but 20,480
  additional logical bytes of stack scratch. Some varied-RGB workloads slow
  down materially, so unconditional B is rejected.
- **C: adaptive selection.** For regions of at least 4,096 pixels and palettes
  with at least 8 entries, inspect at most 128 visible samples, using a bounded
  128-key stack buffer. Choose B only after observing at least 64 samples and
  at most 75% distinct colors; otherwise choose A. No allocations for the
  chooser, and no source pixels are changed by sampling. The cache always
  falls back to exact A on misses. Selection is a performance heuristic, not
  a difference in the algorithmic meaning of quantization.

## Measured native full-operation timings

Rust 1.99.1, release builds, seven alternating A/B and B/A rounds. Full task
includes result allocation, image construction and `png::encode_compressed`;
not filesystem I/O. The recorded A/B results include the same input pixels,
alpha distribution, palette order and PNG encoding settings. All PNG bytes
agree. Figures are median **milliseconds**.

| Platform | 256² repeated, 64 colors A / B | 256² varied, 64 colors A / B | 1024² repeated, 16 colors A / B | 1024² varied, 16 colors A / B |
|---|---:|---:|---:|---:|
| Linux x86-64 | 6.075 / 0.595 | 7.424 / 9.686 | 25.703 / 6.576 | 46.403 / 65.749 |
| macOS ARM64 | 1.945 / 0.236 | 2.862 / 3.058 | 12.250 / 3.987 | 29.591 / 34.147 |
| Windows MSVC | 5.309 / 0.560 | 6.492 / 7.325 | 23.297 / 6.286 | 45.709 / 55.477 |

[Completed three-platform A/B workflow](https://github.com/SirElderling/raipixel/actions/runs/39073130934).

With A/B/C in the same runner and source revision, the adaptive full-operation
results for 256², 64 colors are:

| Platform | Repeated A / B / C (ms) | Varied A / B / C (ms) |
|---|---:|---:|
| Linux x86-64 | 4.857 / 0.391 / 0.387 | 6.299 / 6.921 / 6.232 |
| Windows MSVC | 4.348 / 0.545 / 0.539 | 5.474 / 5.727 / 5.457 |
| macOS ARM64 | Pending native run | Pending native run |

[Adaptive native A/B/C run](https://github.com/SirElderling/raipixel/actions/runs/39073528648).
The macOS runner is still queued as of this decision record; the adaptive
cross-platform gate is not yet fully closed. The Linux and Windows quantize
tests, exact PNG comparisons and native CI passed on the implementation
commit; final Mac verification must be checked before unqualified acceptance.

A also remains the selected fast path for tiny sprites, where C's sampling
would cost more than the saved comparisons. At 64² with 16 repeated colors,
the Linux complete-task C/A ratio is approximately 0.24; for varied colors,
approximately 1.01. At 1024², 16 repeated colors, C/A is approximately
0.26; for varied colors approximately 1.01. Thus C exploits repetition
without the large varied-image regressions measured for unconditional B.

The **20,480-byte cache size is logical stack scratch**, not measured peak RSS;
a maximum 128×4-byte sampling buffer and normal call-stack overhead are
additional. Total process peak memory and pathological input throughput
remain to be measured. Only native image data is changed; the PNG codec,
PNG byte convention and transparent RGB semantics stay intact.

Reproduce with:

```sh
cargo run --release --locked --offline --example bench_quantize
cargo test --locked --offline --test quantize --test cli_quantize
```

Do not introduce an even more elaborate spatial color index, different
distance metric, approximate quantizer, dynamic allocation cache or dithering
until a representative agent-artwork workload shows concrete need.
