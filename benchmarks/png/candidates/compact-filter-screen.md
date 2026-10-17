# Compact PNG plus sampled scanline filters — 16 October 2026

## Decision: do not integrate into production

Keep `png::encode_compact`'s existing simple first-row Sub, remaining-row Up
policy. A new experiment combining opt-in **lazy matching** with sampled
Sub/Up filter selection saves only 3–7% on two 1024px sprite-sheet inputs,
while adding approximately 23–69% whole-writer time on those sheets.
Neither texture nor noise becomes smaller; noise takes approximately 2–2.5×
as long. This does not clear the KISS bar for another filter-scoring loop.

This **does not reject** the already implemented `encode_compact` feature:
its one-step lazy matching is retained. `encode_compressed` remains the
byte-stable fast policy and `encode` remains the original stored writer.

## Method

[Scratch trial](../../../tools/png_compact_filter_trial.py) constructs three
temporary copies of our current PNG implementation, changing only the
`COMPACT=true` row-filter policy. They compare:

- Accepted compact: first row Sub; remaining rows Up.
- All Sub on compact.
- Choose Sub or Up by signed residual score at four points across each row,
  sampling 16 or 64 bytes at each point.

Each candidate is decoded by our accepted PNG decoder and must reproduce
all RGBA bytes, including hidden RGB, as well as the original work accounting.
Nine rotating timed batches are calibrated to at least 25 ms and include the
whole compressed writer and fresh output allocation. **This is a screening
benchmark, not a file-size prediction.** All candidates preserve the PNG
format, use safe stable Rust **1.99.1**, and do not inspect any third-party
`png` crate implementation.

Native run:
[38864190052](https://github.com/SirElderling/raipixel/actions/runs/38864190052).

## Measurements

Relative to accepted compact writer (smaller is better):

| Input | Accepted bytes | Sample 16 bytes | Sample 64 bytes | Linux timing 16/64 | Windows timing 16/64 |
|---|---:|---:|---:|---:|---:|
| Icon 64 | 326 | 317 | 317 | 2.014× / 4.405× | 1.731× / 3.616× |
| Sheet 1024 | 33,463 | 32,355 | 31,911 | 1.280× / 1.578× | 1.233× / 1.532× |
| Mixed-filter sheet 1024 | 38,788 | 37,379 | 36,048 | 1.294× / 1.646× | 1.311× / 1.687× |
| Texture 1024 | 77,133 | 77,133 | 77,133 | 1.091× / 1.258× | 1.068× / 1.199× |
| Noise 256 | 262,560 | 262,560 | 262,560 | 1.895× / 2.303× | 2.104× / 2.527× |

Linux and Windows jobs passed. macOS ARM64 was still queued when this record
was written; do not claim ARM64 parity or superiority from these measurements.

The unconditionally all-Sub variant was worse: 60,269 bytes vs 33,463 on
the generated sheet and 63,841 vs 38,788 on the original mixed-filter sheet,
at roughly 4–5× the accepted compact writer's runtime.

## Consequences

Do not add another public compression mode, automatic filter-policy switching,
or more per-row work in the compact writer based on these results. Better
compression size parity against the reference's default compressor is a
separate question that would require substantially different DEFLATE coding
or indexed/palette output, guided by a representative pixel-art corpus.
Experiments should also respect per-input budget charging before publication.

No production codec was changed by this trial, and the project retains zero
runtime dependencies, no unsafe Rust, deterministic pixels and the original
fast/stored byte contracts.
