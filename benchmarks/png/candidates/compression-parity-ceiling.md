# PNG compression parity: isolated DEFLATE coding and palette headroom

Date: 16 October 2026. All accepted raipixel source remains safe stable Rust 1.99.1 with no
runtime dependencies. These are *diagnostic* tests, not a substitute
for a native production implementation or a speed-parity claim.

## Method

[`tools/png_compression_ceiling.py`](../../../tools/png_compression_ceiling.py)
decodes representative PNGs with our accepted decoder, writes the accepted
`encode_compact` result, and extracts its zlib-inflated **exact same filtered
RGBA scanlines**. Python's **standard-library** `zlib` is then used to generate
alternative DEFLATE streams. No `png` crate implementation source is opened or copied.

For the coding test, all settings other than `Z_FIXED` versus
`Z_DEFAULT_STRATEGY` are held equal (zlib level 6, same filtered input).
Full-stream versus `Z_FULL_FLUSH` after every 65,535 input bytes is an
*approximation* of the existing raipixel block-local match history; flushes
themselves add DEFLATE bytes, so it is not an exact ablation.

For indexed-color tests, PNG PLTE and tRNS entries are derived from **distinct
four-channel RGBA values**, including RGB hidden behind alpha zero. Both
8-bit indexed and legal packed 1/2/4-bit palettes are tested when feasible,
with PNG None scanline filters. Before reporting, every trial is decoded by
the unmodified raipixel decoder and required to reproduce every pixel exactly.

The initial [native run 38869423791](https://github.com/SirElderling/raipixel/actions/runs/38869423791)
passed on Linux x86-64, Windows x86-64 and macOS ARM64: the exact same stream
and PNG bytes are produced on all three. The extended coding/depth sweep
[run 38869796501](https://github.com/SirElderling/raipixel/actions/runs/38869796501)
passed on Linux and Windows; macOS was queued when this record was written.

## Results: fixed versus dynamic Huffman, identical level 6 and scanlines

| PNG input | Current compact bytes | zlib level 6 fixed codes | zlib level 6 dynamic codes | Dynamic / fixed |
|---|---:|---:|---:|---:|
| Generated sprite sheet 1024 | 33,463 | 30,172 | **8,191** | 0.271 |
| Original mixed-filter sheet 1024 | 38,788 | 32,590 | **10,891** | 0.334 |
| Generated texture 1024 | 77,133 | 34,793 | **12,564** | 0.361 |
| Noise 256 | 262,560 | 262,548 | 262,548 | 1.000 |

**Decision:** A dynamic-Huffman writer is a genuine, high-upside strategy
for reducing structured-image sizes. It is not a trivial patch: the writer
needs frequency counting, bounded Huffman length construction, canonical
code assignment and code-length RLE according to RFC 1951, with malformed
stream/fuzz tests and portable native timing. The underlying zlib matcher
also differs from ours: *the fixed-vs-dynamic rows isolate Huffman coding
only within zlib's matcher*, not within our own compressor. It would be
incorrect to promise the same output sizes or speeds from the raipixel writer.

The 65,535-byte `Z_FULL_FLUSH` approximation increases output from 8,191 to
10,447 bytes on the generated sheet and from 12,564 to 16,659 bytes on the
texture with default dynamic coding. Preserving a 32 KiB rolling history
across boundaries also has upside, but the size ceiling from coding choice
is larger on this corpus.

## Results: exact RGBA indexed output

| PNG input | Current compact RGBA | Indexed 8-bit + zlib level 6 | Packed indexed, minimal depth + zlib level 6 |
|---|---:|---:|---:|
| Icon 64, 4 RGBA colors | 326 | 156 | **135 B** (2-bit) |
| Sheet 1024, 4 RGBA colors | 33,463 | 5,520 | **2,839 B** (2-bit) |
| Original sheet 1024, 5 RGBA colors | 38,788 | 5,541 | **3,525 B** (4-bit) |
| Texture 1024, >256 RGBA colors | 77,133 | Not representable | Not representable |
| Noise 256, >256 RGBA colors | 262,560 | Not representable | Not representable |

**Decision:** Indexed PNG output is highly relevant to pixel art, but
it must be **conditional and lossless**: reject/fall back when more than
256 exact RGBA colors occur; never quantize away hidden RGB values. Palette
indexing, 1/2/4/8-bit packing and tRNS require independent interoperability
checks and a realistic encoding time measurement.

[`tools/png_palette_native_trial.py`](../../../tools/png_palette_native_trial.py)
therefore builds a temporary **safe Rust** indexed writer using **our own**
`deflate::Encoder::block_compact` (not Python zlib) and compares complete
file size/encoding time, sample equality, CRC validation, deterministic
output, and budget failure against accepted compact RGBA output.
Its [native run 38870381490](https://github.com/SirElderling/raipixel/actions/runs/38870381490)
is queued; no native indexed production change has been accepted.

## Decoder parity ceiling

PNG noise decode is dominated by CRC32 work on several prior native profiles.
A further [`crc32fast` public-API screen](../../../tools/png_crc_reference_api.py)
now measures our CRC against the exact `crc32fast 1.5.2` locked into the
reference `png 0.18.1` dependency graph. This comparison uses a temporary
package and **no implementation inspection**; it does not authorize a
production dependency, intrinsics or unsafe Rust. Results are pending in
the Rust 1.99.1 workflow. No claim of full decoder-speed parity is made.

## Implementation priority after the screens

1. If the independent native indexed writer has meaningful size savings at
   acceptable time/memory cost, promote a narrowly scoped **opt-in exact
   indexed PNG writer** with minimal legal bit depth. Keep the existing
   RGBA8 stored/fast/compact output contracts untouched.
2. Prototype a standalone **dynamic-Huffman DEFLATE writer** as an isolated
   RFC 1951 component, benchmarked against our compact writer and the locked
   public reference. Reject if build size, complexity, speed or correctness
   regressions outweigh compression-size gains.
3. Treat decoding checksum parity separately: maintain complete CRC/Adler
   validation and safe Rust; document the actual ceiling versus the reference
   accelerated checksum dependency and do not promise accelerated-path parity
   without evidence.

The existing benchmark corpus is deterministic and not representative of
all real-world pixel-art assets; expand it before generalizing these numbers.
PR #2 remains open and unmerged.
