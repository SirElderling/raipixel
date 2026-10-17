# Size-focused PNG writing: native results and decision (16 October 2026)

## Decision

Keep two explicit lossless compression policies rather than slowing every
export:

- `png::encode_compressed`: retain the original **fast** greedy parse and
  exact existing output bytes.
- `png::encode_compact`: use one-step lazy match selection with the same
  bounded dictionary, fixed Huffman output, Sub/Up row filters, and stored
  fallback. The new mode can trade extra encoding time for fewer bytes.

This is an opt-in size policy, **not** general parity with the reference
crate's default compressor. The reference uses a different tradeoff and may
produce much smaller outputs. Both policies preserve exact RGBA samples,
including hidden RGB, and deterministic PNG outputs.

The feature implementation is in PR #2. Core tests check no-dependency,
safe-Rust output, repeated byte equality, full roundtrip, stored bounds,
resource-budget preflight, and multi-block/noncompressible cases. A separate
optional harness verifies profile fixtures and emitted PNGs through Pillow and
the locked `png 0.18.1` public decoder. Preserve the preexisting fast/stored
golden-byte contracts. Latest complete native CI for these changes should
be reviewed before merging; this record is not an all-green claim.

## Independent native candidate screen

[`tools/png_match_depth_trial.py`](../../../tools/png_match_depth_trial.py)
compares greedy (existing), one-step lazy, two hash-history entries, and
both, with **the same image** and compiler/profile in each process. Every
candidate emits deterministic PNG bytes and decodes to the same exact RGBA8.
Nine rotating 25-ms-minimum batches include fresh compression allocation.
Source is our own safe-Rust encoder; third-party implementation code was
never inspected.

[Native experiment 38859754646](https://github.com/SirElderling/raipixel/actions/runs/38859754646)
passed on Linux x86-64, Windows x86-64 and macOS ARM64 with Rust 1.99.1.

| Source pixels | Current PNG bytes | Lazy bytes | Two-entry bytes | Both bytes |
|---|---:|---:|---:|---:|
| Icon 64×64 | 388 | 326 | 315 | 315 |
| Generated sheet 1024² | 38,114 | 33,463 | 32,251 | 32,094 |
| Original mixed-filter sheet 1024² | 52,549 | 38,788 | 36,864 | 36,003 |
| Texture 1024² | 83,186 | 77,133 | 63,067 | 65,128 |
| Noise 256² | 262,560 | 262,560 | 262,560 | 262,560 |

Whole-writer runtime ratios **lazy/current** (lower is faster):

| Workload | Linux | Windows | ARM64 |
|---|---:|---:|---:|
| Icon 64 | 1.014 | 1.034 | 1.091 |
| Generated sheet 1024 | 0.993 | 1.017 | 0.991 |
| Original mixed-filter sheet | 0.935 | 0.936 | 0.978 |
| Texture 1024 | 1.279 | 1.263 | 1.268 |
| Noise 256 | 1.007 | 1.004 | 1.123 |

The faster or neutral sprite-sheet results do not imply a universal speed
improvement. Textures have repeatable ~26–28% time regressions, and ARM64
noise is ~12% slower. A separate opt-in mode makes these costs explicit.

The two-entry hash mode uses a 64 KiB dictionary instead of 32 KiB and may
improve textures by 24% in size. But in the same native measurements it is
~11–38% slower on most compressible sources and saves only ~5% more on the
original sheet than lazy alone. Combining both increases runtime further.
**Do not add another public mode or enlarge the default dictionary based on
this screen**; keep the simpler one-step path.

## Rejected broad filter change

[`tools/png_filter_policy_trial.py`](../../../tools/png_filter_policy_trial.py)
tested all-Sub and sampled Sub/Up row decisions against the accepted fast
encoder. [Three-platform run 38859303297](https://github.com/SirElderling/raipixel/actions/runs/38859303297)
passed all valid PNG pixel and work-accounting checks.

On the original mixed-filter 1024² sheet, sampled 16/64-byte policies produced
0.818×/0.698× the current output size, but took 1.21–1.33×/1.65–1.70× as
long across the three runners. On noise they gave no size benefit and took
~1.86–3.16× as long. All-Sub regressed generated-sheet size to 1.73×.
These results reject unconditional adaptive filtering in the **fast** path.
Do not assume that combining individually measured approaches yields additive
benefits without a separate native benchmark.

## Remaining gap

Raipixel still emits **fixed Huffman** DEFLATE streams; the default
`png 0.18.1` writer can be more size-efficient. The next substantially
different compression-size approach would involve dynamic-code selection or
palette-aware pixel-art output, neither of which is justified as a small
patch by the current screens. Evaluate an independent real sprite/tile corpus
and the default reference output sizes before expanding the implementation.

## CRC decode experiments

Four separate three-platform runs of valid full-PNG threshold screens
([38856408740](https://github.com/SirElderling/raipixel/actions/runs/38856408740),
[38859303297](https://github.com/SirElderling/raipixel/actions/runs/38859303297),
[38859754646](https://github.com/SirElderling/raipixel/actions/runs/38859754646),
[38861694290](https://github.com/SirElderling/raipixel/actions/runs/38861694290))
tested an immediate switch to the already deployed wide CRC slicing loops.
CRC-only 256B–2KiB cases were about twice as fast, but end-to-end decoding
was mostly within a few percent; ARM64 cases sometimes regressed by 5–14%
in individual repetitions. It is not credible to claim broad decode gains.
**Retain the current 4096-byte cutoff** absent repeatable whole-codec evidence.
The experimental CRC-disabled build measures headroom only; it must never
enter production.

All experiments use stable Rust 1.99.1; no unsafe Rust, runtime dependencies,
or inspection of the reference crate's implementation.
