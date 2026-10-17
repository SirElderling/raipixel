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

## Native Rust proof-of-concept: passed Linux and Windows

A further [focused native Rust workflow](https://github.com/SirElderling/raipixel/actions/runs/38897607257)
compiles [`tools/png_dynamic_native_trial.py`](../../../tools/png_dynamic_native_trial.py),
which makes temporary, test-only copies of our **own** `src/deflate/encode.rs`.
It records exactly the same literal, length and distance tokens already selected
by raipixel, emits RFC 1951 dynamic Huffman blocks, and selects them only when
smaller than the accepted fixed/stored block. Tree construction is bounded by
DEFLATE's maximum bit lengths; otherwise it falls back. It does not add an
external codec, runtime crate dependency, `unsafe` or new production mode.

Validation before timing includes **exact RGBA image equality** via our accepted
decoder, deterministic repeat-encoding bytes, work-budget parity, and CRC
corruption rejection. Nine rotating calibrated batches time whole PNG encoding
including allocation and token buffering.

| Workload | Accepted bytes | Native dynamic bytes | Windows encode time | Linux encode time |
|---|---:|---:|---:|---:|
| Icon 64 | 326 | 289 | 2.25× | 2.19× |
| Generated sheet 1024 | 33,463 | **12,949** | **1.70×** | **1.71×** |
| Original mixed-filter sheet | 38,788 | **19,449** | **1.90×** | **1.91×** |
| Texture 1024 | 77,133 | **55,766** | **1.95×** | **1.89×** |
| Noise 256 | 262,560 | 262,560 | 1.37× | 1.32× |

Relative times above **1.0× mean slower**. Linux and Windows passed with rustc
1.99.1. The ARM64 job was queued at time of writing and is not included in
this conclusion.

The native PNG output is 792 bytes larger than the single-IDAT Python diagnostic
on the 1024px images because it retains the existing multi-IDAT chunk framing;
the DEFLATE content is otherwise comparably compact. This is intentional
for the bounded streaming-style writer.

**Decision:** Dynamic coding is feasible with our *actual matcher*, but the
naive scratch strategy re-encodes a fixed block before building the dynamic
alternative, and repeatedly sorts Huffman nodes. Its **70–95% more CPU on
structured images** and 30% more CPU on incompressible noise do not justify
replacing the existing fast or compact encoders. Keep this prototype isolated
and evaluate a **single-pass token-buffered dynamic writer** (or explicit
size-focused mode), measuring code size, scratch memory and end-to-end time
before any production integration. Existing output byte contracts remain intact.
