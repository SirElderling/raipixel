# Parity review: code generation, SIMD and remaining work

Reviewed 2026-10-15 at production-source revision 8fed9fa, unchanged through
the diagnostic commit a2f646f. All implementation and code-generation inspection
concerns our own source. No reference-crate implementation was inspected/copied.

## Target and evidence

The [latest complete public-API comparison](../run-match-adler-final.json) gives
the original 1024 sheet 1.177 ms versus `png 0.18.1` Fast 0.797 ms: our time
must fall another **32.2%**, not 47.6%, to equal that observation. Input decode
is 1.438 versus 0.857 ms and needs a separate 40.4% time reduction. Feature,
compression-size and speed parity are separate targets; the current codec does
not offer 16-bit samples/APNG or a dynamic-Huffman writer.

Fast encoding time ratios (ours/reference), same recorded local comparison:

| Input | Ratio | Our / Fast PNG bytes |
|---|---:|---:|
| icon-16 | 1.306 | 228 / 279 |
| icon-32 | 1.159 | 300 / 355 |
| icon-64 | 1.078 | 479 / 470 |
| sheet-256 | 1.310 | 3,908 / 3,911 |
| sheet-1024 | 1.476 | 52,549 / 53,405 |
| texture-256 | 0.246 | 5,734 / 207,670 |
| texture-1024 | 0.243 | 83,186 / 3,316,027 |

The texture results do not establish general superiority: filters and compression
choices differ and reference Balanced produces much smaller files. Measure the
same complete public operations on the same inputs, with identical build/CPU
settings for both libraries. Preserve allocation, integrity checks, work limits,
pixel checks and the noise regression case. Native before/after CI is not yet
a native three-platform comparison against the crate itself.

## What the earlier work rules out

- Retained: CRC slicing, packed Adler grouping/reduction, distance-one inflate
  fill, RGBA allocation reuse, run-first early exit, direct length/distance code
  mapping, compact fixed codes, bounded 16-byte matches and a small rejection
  guard. See the preceding candidate records for raw measurements and limits.
- Rejected: original copy/filter/checksum fusion, XOR mismatch termination,
  extra-bit packing and larger/interleaved accumulator variants without a
  repeatable complete-codec gain. Do not repeat an identical rejected variant.
- Deferred: decoder Huffman table redesign; measured construction costs are too
  small on these fixtures to explain the decode gap. A more elaborate encoder
  parse or dynamic Huffman writer addresses different size/speed tradeoffs.

The [actual writer profile](../writer-profile-match-adler.json) still points to
DEFLATE (about 0.681 ms) and Adler (0.354 ms), followed by filtering (0.178 ms).
Its plain/profiled totals are 1.185/1.286 ms. These instrumented medians cannot
be subtracted as exact production savings. Even halving Adler alone would not
close the complete-writer gap in that profile.

## Inspecting our generated instructions

Rust 1.99 / LLVM 23.1.1, Linux x86-64 release assembly of our library:
`cargo rustc --lib --release --offline -- --emit=asm,llvm-ir`.

- `matching_tail` already emits `movdqu`, `pcmpeqb`, `pmovmskb` for each group:
  safe Rust already obtained SSE2 SIMD. The group loop has no per-byte bounds
  checks. Slice-range failure paths remain outside the group loop.
- `Adler::update` uses scalar word arithmetic and has no bounds-panic paths in
  this generated function. Replacing indexing with unchecked accesses does not
  remove a bounds check from this bulk loop.
- Encoder block code still has checks and byte-at-a-time output pushes. Counting
  static panic paths is not measuring their runtime cost; our environment lacks
  `perf`, so this is a code audit, not a hardware-counter profile.

Thus no measured evidence currently justifies broad unsafe indexing or assembly.
SIMD checksum algorithms remain a real candidate. LLVM's
[vectorization documentation](https://llvm.org/docs/Vectorizers.html) explains
the cost model, reductions and register-pressure tradeoffs; emitting vector
instructions alone does not prove that they are faster.

## New safe screens

`tools/png_parity_screen.py INPUT.png [...]` builds temporary variants of our
encoder and `tools/png_adler_lanes_trial.rs`. Nine rotating batches calibrate
each operation to 25 ms. PNG extraction, equality/scalar validation and I/O are
untimed; fresh encoder/output allocation is included. These are isolated stages,
not complete PNG calls or crate comparisons. No prototype is selected in the
production codec. Source hashes, compiler/flags and every sample are recorded.

The encoder variants preserve complete DEFLATE byte equality before timing:

1. **Narrow dictionary:** retain all 8,192 slots but store u16 positions. For a
   65,535-byte block, every stored start is <=65,532; 65,535 is an unused sentinel.
   This halves dictionary storage/reset traffic, 32 to 16 KiB, without changing
   hashing, candidates or tie-breaking. Less traffic is not a guaranteed win.
2. **Constant-run scan:** compare groups with `[preceding_byte;16]` rather than
   comparing two overlapping source slices for distance-one matches. Preserve
   minimum length, maximum cap and every token choice.
3. Both together, to expose interactions rather than assuming additive gains.

[Native CI run 38766587687](https://github.com/SirElderling/raipixel/actions/runs/38766587687)
passes 62 core tests, formatting, Clippy and all new scalar/exact-byte screens
on Linux, macOS and Windows. Stage time ratios (candidate/current):

| Variant / input | Linux | macOS ARM64 | Windows |
|---|---:|---:|---:|
| u16 dictionary / sheet | 0.985 | 0.984 | 0.921 |
| constant-run / sheet | 0.894 | 0.814 | 0.881 |
| combined / sheet | 0.875 | 0.787 | 0.816 |
| combined / texture | 1.029 | 0.926 | 0.999 |
| combined / noise | 1.040 | 1.014 | 0.985 |

Raw reports: [Linux](../parity-screen-native-linux.json),
[macOS](../parity-screen-native-macos.json),
[Windows](../parity-screen-native-windows.json). These generated images differ
from the original comparison corpus. The original-corpus local
[generic run](../parity-screen-local-generic.json) instead has the narrow
dictionary alone 3% slower for the sheet, and combined noise 28% slower. Keep
that result visible; no candidate is accepted merely for a favorable CI sample.

The Adler prototype accumulates independent byte columns and their prior sums
over <=4,096-byte groups, then reduces once per group. For width W, m complete
rows, S[i]=sum(x[k*W+i]), P[i]=sum((m-1-k)*x[k*W+i]):

```text
A' = A + sum(S[i])
B' = B + m*W*A + W*sum(P[i]) + sum((W-i)*S[i])
```

The remaining <W bytes use the defining scalar recurrence. W=8/16/32 uses u32
columns/u64 totals; at W=8 each sum <=130,560 and prior sum <=33,358,080, so
neither column overflows. The checksum is reduced modulo 65,521 per group.
Validation compares all variants with a scalar checksum on actual inputs,
all-255/patterned data and lengths surrounding the new group/lane boundaries.
This one-shot diagnostic is not an incremental production replacement.

With default generic code generation it **loses**: the best native sheet variant
is 1.31x/2.27x/1.25x current time for Linux/macOS/Windows. Do not adopt it.
With [local AVX2, 16 codegen units](../parity-screen-local-avx2.json), the best
sheet variant is 0.339 versus current 0.537 ms, but wider variants regress.
With [explicit AVX2, one codegen unit](../parity-screen-local-avx2-cgu1.json),
the 32-column sheet is 0.147 versus current 0.507 ms. Both operations within
each executable have identical flags. Neither result is portable parity.

Early standalone probes requested assembly as well as an executable; rerunning
with executable-only emission changed the lane timings substantially. Explicit
codegen-unit trials reproduce this sensitivity. The permanent runner now records
and explicitly sets codegen units (default 16), and permits `--codegen-units=1`,
`--features=+avx2`, or `--cpu=native` as diagnostics. The first native run uses
rustc's implicit default of 16; its raw flags are preserved unchanged. Do not
mix these runs into a same-settings speedup claim. Inspecting generated assembly
and benchmarking must use matched build settings. Cargo
[profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) and rustc's
[codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html#codegen-units)
document these optimization controls. The production manifest is unchanged.

The initial Windows screen's Python default encoding reads UTF-8 author names
in comments as CP1252 before hashing the resulting string as UTF-8. Writing
those strings back with the same locale preserves the compiler input bytes;
the recorded string hash differs. Reproducing that decode/re-encode matches
the recorded fingerprint. The runner now explicitly reads/writes UTF-8. The
raw Windows report is retained, not silently normalized to a different hash.

## Next experiments, in order

1. **Complete-codec build matrix:** ordinary release versus one codegen unit and
   thin LTO, identical settings for ours and the locked reference, with byte,
   size, allocation and native checks. This is a real release-profile choice,
   not permission to report one optimized library against an unoptimized one.
2. **Checksum SIMD:** make the column kernel predictable under the chosen build
   profile, then measure complete encode/decode/stored operations. Check ARM64
   as well as x86-64; a CPU-native build alone is not a portable distribution.
3. **Constant-run/dictionary layout:** native stage results justify a complete
   writer trial with current exact hashes and the texture/noise regressions kept
   visible. The u16 change is attractive for memory even where its speed is flat.
4. **Emitter:** separately try a wider bit reservoir and batched safe slice
   writes. Earlier extra-bit packing was a different rejected experiment. Token
   replay suggests a smaller ceiling here than in matching/checksum work.
5. **Decoder:** bulk bit refill and bounded overlap copying after measuring the
   loop costs, retaining truncation, work-limit and checksum behavior. Encoder
   improvements alone cannot establish decoder parity.

If those safe changes are insufficient, targeted `std::arch` checksum/filter
intrinsics behind a safe API are a reasonable next proposal. They keep stable
Rust and zero dependencies but may require a narrowly scoped unsafe boundary
for loads/CPU feature dispatch, thus changing the current no-unsafe policy.
Rust's [arch documentation](https://doc.rust-lang.org/std/arch/index.html)
requires the correct architecture and supported CPU features; retain a portable
fallback. [Portable SIMD](https://doc.rust-lang.org/std/simd/index.html) is still
nightly-only in Rust 1.99 and does not satisfy this project's stable requirement.

`unsafe` is not an optimization switch: regular references retain their checks,
as explained in [The Rust Book](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html).
Hand-written assembly is deferred until a measured kernel beats both compiler
output and intrinsics enough to justify separate x86-64/ARM64 maintenance. No
assembly or unsafe implementation is added by this review. No guarantee that
any candidate or combination reaches parity follows from these stage screens.

## Attribution and output contract

The column algebra is independently derived from our existing grouping and the
Adler/Gailly recurrence in L. Peter Deutsch and Jean-Loup Gailly's RFC 1950
(1996), sections 2.2/6. The dictionary and distance-one scan follow L. Peter
Deutsch's RFC 1951 (1996), sections 3.2.3/3.2.5/3.2.6; bounded-matcher conceptual
credit to Bille, Cording, Fischer and Gørtz (CPM 2017) remains as documented in
[algorithm notes](../algorithm-notes.md). No new paper algorithm or external
implementation is adopted. LLVM/Rust documentation informs compiler and safety
decisions, not a copied checksum/matcher implementation.

Exact RGBA samples, including hidden RGB, remain mandatory. These prototypes
also preserve the existing complete encoded bytes. A future change to parsing,
filters, block selection or dynamic coding can preserve pixels while changing
PNG bytes; that is a separate compression-policy experiment with explicit size
and determinism checks, not a way to hide a speed/size regression.
