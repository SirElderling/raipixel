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

**Accepted in PR #2:** The public `png::encode_indexed` writer is now an
**explicit opt-in**, preserving all four RGBA channels, including hidden RGB
under alpha zero, with the minimal 1/2/4/8-bit legal index depth. It uses
the existing bounded, fixed-Huffman lazy encoder, a deterministic, bounded
4096-slot palette lookup and the same checksum validation. An image with
more than 256 distinct RGBA values returns `Ok(false)` without writing,
rather than silently quantizing. The existing `encode`, `encode_compressed`
and `encode_compact` byte contracts remain unchanged.

A native [three-platform scratch screen 38872957346](https://github.com/SirElderling/raipixel/actions/runs/38872957346)
demonstrated the real fixed-Huffman palette ceiling, without borrowing
Python zlib:

| PNG input | Current compact RGBA bytes | Own fixed-Huffman optimal indexed bytes | Indexed / compact |
|---|---:|---:|---:|
| Icon 64 | 326 | **172** | 0.528 |
| Sheet 1024 | 33,463 | **4,396** | 0.131 |
| Original mixed-filter sheet 1024 | 38,788 | **9,912** | 0.256 |

The indexed prototype with bounded fixed-table lookup took approximately
3–4.6 times as long as `encode_compact` for the two sheets across Linux,
Windows and ARM64. These are **scratch candidate times**, not a claim about
later production measurements. Palette conversion is a deliberate
compression-size versus CPU tradeoff, not a new universal default.
In particular, a 9,912-byte fixed-Huffman indexed sheet remains larger
than a 3,525-byte zlib-compressed indexed diagnostic: **dynamic Huffman
encoding still has meaningful upside even after palette conversion**.

Production tests cover 1/2/4/8-bit palettes, RGBA values with hidden RGB,
deterministic exact output, 65,535-byte block boundaries, invalid checksums,
too many colors, and failed work preflight. The independent
[`png 0.18.1` public decoder](../../../tools/png_compare.rs) is now
also used for exact indexed-output checking in the matched-profile CI.
Its latest run must finish successfully before considering independent
interoperability established; the reference's source code is never read.

## Decoder parity ceiling

PNG noise decode is dominated by CRC32 work on several prior native profiles.
A further [`crc32fast` public-API screen](../../../tools/png_crc_reference_api.py)
now measures our CRC against the exact `crc32fast 1.5.2` locked into the
reference `png 0.18.1` dependency graph. This comparison uses a temporary
package and **no implementation inspection**; it does not authorize a
production dependency, intrinsics or unsafe Rust. Results are pending in
the Rust 1.99.1 workflow. No claim of full decoder-speed parity is made.

## Implementation priority after the screens

1. Verify the accepted **opt-in exact indexed PNG writer** with the
   independent reference decoder, and measure its native CPU/memory cost.
   Keep existing RGBA8 stored/fast/compact output contracts untouched.
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
