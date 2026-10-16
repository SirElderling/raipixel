# Dynamic Huffman, identical raipixel LZ tokens — native parity screen

Date: 16 October 2026. Compiler rustc 1.99.1. Experiment only; accepted codec unchanged.

## Result: dynamic coding is demonstrably useful with our matcher

[Run 38896760492](https://github.com/SirElderling/raipixel/actions/runs/38896760492)
passed on Linux x86-64, Windows x86-64 and macOS ARM64, with exactly identical
candidate sizes and all reconstructed PNGs verified by **Python zlib** and
**raipixel's production PNG decoder** for all four RGBA channels, including
RGB hidden under zero alpha.

[`tools/png_dynamic_tokens_trial.py`](../../../tools/png_dynamic_tokens_trial.py)
parses the actual accepted `encode_compact` output, decoding **only our own
DEFLATE fixed and stored blocks** into the existing literal, length and distance
tokens. A standalone independent Python writer reconstructs the original
fixed/stored bitstream byte-for-byte before testing any change. It then
re-emits the **exact same tokens and block structure** with RFC 1951 dynamic
Huffman trees on blocks where dynamic bits beat fixed bits. The zlib header,
Adler32 and all filtered scanline bytes are unchanged. The output is checked
against the input RGBA using the production raipixel decoder, not inferred
from file size. This isolates dynamic code choice from match search.

| PNG workload | Accepted compact PNG | Re-emitted dynamic PNG | Reduction | Dynamic blocks |
|---|---:|---:|---:|---:|
| Icon 64 | 326 B | 265 B | 18.7% | 1/1 |
| Sheet 1024 | 33,463 B | **12,157 B** | **63.7%** | 64/65 |
| Original mixed-filter sheet 1024 | 38,788 B | **18,657 B** | **51.9%** | 64/65 |
| Texture 1024 | 77,133 B | **54,974 B** | **28.7%** | 64/65 |
| Noise 256 | 262,560 B | 262,488 B | No DEFLATE-code benefit | 0/0 |

The small total-file change for noise is **PNG chunk-framing savings** from
consolidating original multi-chunk IDAT into one IDAT in the diagnostic.
The DEFLATE stream itself does not improve. Avoid claiming noise compression
benefit from this test.

These results still do not equal the zlib level-6 best (e.g. sheet 8,191 B,
texture 12,564 B), showing **match-search quality and cross-block history**
remain relevant. A dynamic tree encoder is *necessary but not sufficient*
for complete default-compression parity.

## Decision gate for native Rust

Develop a **scratch-only** safe-stable-Rust dynamic writer that uses the exact
same accepted tokenization, adds bounded per-block symbol frequency counting,
canonical Huffman lengths/codes, RFC 1951 code-length RLE and a size-based
fallback to the old block. Test across 1/2/4/8-bit palettes, both RGBA writers,
stored/noise blocks, invalid CRCs, boundary lengths and native decoding.
Measure full encoding time **including token buffering/allocations**, memory
and code footprint. Avoid committing additional public modes or changing the
stored/fast/compact byte contracts until the native candidate clears these
gates. The reference `png 0.18.1` implementation remains black-box only,
and no unsafe code/runtime crate dependencies are permitted.
