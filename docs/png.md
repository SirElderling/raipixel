# Static PNG codec

This implements the Foundation import profile in the design, not full PNG
conformance or completion of S1. The library exposes `png::decode` and
`png::encode` and optional `png::encode_compressed`; typed jobs, persistence and the CLI remain separate work.

## Format coverage

| Capability | Implemented behavior |
|---|---|
| Static input | Gray, RGB, indexed, gray-alpha and RGBA at 8 bits; gray/indexed at 1/2/4 bits |
| Row encoding | All five filters; noninterlaced and Adam7, including empty passes and pass-local previous rows |
| Transparency | Straight RGBA samples, hidden RGB, palette alpha and unscaled tRNS comparisons with unused high bits masked |
| Compression input | Stored, fixed and dynamic Huffman DEFLATE; overlapping references and permitted degenerate alphabets |
| Output | Deterministic noninterlaced RGBA8: original None/stored writer or optional Sub/Up, fixed Huffman LZ matches and stored fallback; bounded IDAT chunks |
| Integrity | Chunk CRC32, zlib header/Adler32, exact inflate length, required critical chunks/order, palette/sample constraints |
| Metadata | Color/profile chunks flagged as uninterpreted; other ancillary chunks flagged as skipped; payloads CRC checked but never inflated or preserved |
| Unsupported | 16-bit input and animation chunks explicitly rejected; unknown critical chunks rejected |

Extra bytes following the zlib checksum in IDAT are counted and ignored, as
specified for PNG. Bytes after IEND are rejected by this application profile.
Unknown ancillary chunks with the reserved name bit set are treated as unknown
ancillary chunks, rather than rejected solely for that bit.

The applicable standards are [PNG Third Edition](https://www.w3.org/TR/png-3/),
[RFC 1950](https://www.rfc-editor.org/rfc/rfc1950) and
[RFC 1951](https://www.rfc-editor.org/rfc/rfc1951). Implementations and fixture
generators were authored from those definitions, not copied decoder code.
The reserved-bit handling follows PNG sections 13.5/15; transparency masking
follows section 11.3.1.1.

## Code organization and bounded work

- `png` owns chunk validation, PNG policy/diagnostics, pass geometry, row
  reconstruction and output framing. It performs no filesystem/process I/O.
- Private `deflate` owns buffered bit input, canonical Huffman lookup tables,
  block parsing, history copies and the zlib wrapper. Its `encode` submodule
  owns bounded hash matching and fixed/stored block emission. It knows no PNG geometry.
- Private `checksum` owns slicing-by-eight CRC32 tables and incremental Adler32.
  Ordinary functions connect the modules; no backend traits or registry exist.

Fixed Huffman lookup tables are constructed at compile time. Dynamic tables
use one direct lookup per symbol and at most 32,768 entries per alphabet;
construction and decoding charge work. History copies grow from existing
output in bounded chunks, preserving overlap without byte-at-a-time pushes.

The input ceiling is 128 MiB. Image dimensions/pixels and each heap buffer are
checked against the supplied raster limits. Pass geometry determines the exact
inflate size before allocation; inflate cannot grow beyond it. A single IDAT
payload stays borrowed; multiple payloads are joined with fallible reservations.
Joined compressed storage is released before reconstruction. Filters are
reversed in place in the inflated buffer. Noninterlaced RGBA8 compacts out
filter bytes in place and adopts that allocation as the image; other color
formats and Adam7 still hold inflated samples and a separate RGBA image.
It is not a streaming-row decoder and has no aggregate project-memory accountant.

The writer uses at most 65,540 scratch bytes and streams IDAT chunks to an
ordinary `Write` sink. Work is preflighted before output. I/O failure may leave
partial bytes in that sink; the future publication layer must stage files.
The original output is valid but deliberately uncompressed. Bulk checksum processing and contiguous row copies preserve the original
encoded bytes while improving measured throughput. The optional compressed writer adds less than 169 KiB of bounded scratch,
independent of dimensions, and precharges a linear work budget before output.
It tries one recent hash candidate plus a distance-one run candidate, limits
length/distance to 258/32,768, resets the dictionary per 65,535-byte block and
uses stored fallback when fixed coding would expand a block. Sub on the first
row and Up thereafter preserve every sample, including hidden RGB. No dynamic
Huffman generation, match chains or input-sized index is introduced.

Algorithm sources, creator acknowledgments and the exact partial influences
are recorded in [the algorithm notes](../benchmarks/png/algorithm-notes.md),
under the [mandatory attribution rule](design/01-research.md#research-attribution-requirements).

## Verification

61 Rust tests run offline with no external executable or test library:
22 raster tests, 19 JSON tests, ten PNG integration tests, six DEFLATE unit tests and four checksum unit
tests. The PNG/DEFLATE cases include 31 accepted PNG files, 40 rejected-profile
files and eight independent zlib streams. Cases cover the required profile
matrix and the malformed-input rules listed in the design, rather than
speculative product capabilities.

Fixtures are generated by original `tests/fixtures/generate_png.py`, using
Python's standard zlib/binascii for independent compression and checksums.
Expected RGBA samples are supplied before encoding. The generator is optional;
Rust tests consume checked-in text fixtures and do not invoke Python.

Independent development checks decoded all 31 files, re-encoded them with this
writer, and consumed the output with an external image decoder. Every RGBA
sample matched, including hidden RGB. Separately generated 16/32/64/256/1024
RGBA images also imported with exact samples. Canonical writer bytes are checked
against independently constructed expected bytes; self-round trips are not the
only evidence. The spec review produced a red checkpoint with two reserved-bit
failures before the corresponding fix.

These fixtures establish useful coverage, not certification or proof of
production robustness. Color management and skipped ancillary semantics are
outside this profile.

## Measure

```sh
cargo run --release --locked --offline --example png_bench -- INPUT.png 25
```

The std-only tool calibrates each operation to at least 25 ms per batch, then
reports seven rotating samples for input/stored decode and stored/compressed
encode. It excludes file I/O and startup, includes fresh allocations and
integrity checks, and observes actual buffers through `black_box`.

```sh
python tools/png_timings.py --baseline 8db0275 --output timings.json
```

This runner compares the same driver/compiler/profile against baseline/current
libraries in ABBA process order, checks complete writer SHA256 equality outside
timing, and records provenance, raw samples and median/range summaries. It runs
natively on all three CI platforms. See the [timing protocol](../benchmarks/png/timing.md).
The separate optional reference comparison retains independent image checks.

A development baseline on shared Linux x86_64, AMD EPYC 9V74, Rust 1.99.0 and
default Cargo release settings measured the following. Inputs were independently
encoded RGBA8 PNGs: an original 16-pixel icon scaled with nearest-neighbor and
repeated into fixed 64-pixel cells. Values are medians of seven batches, with
rotating operation order; 500 iterations/batch for 64 pixels and ten for 1024.
Output allocation and checksums are included; disk I/O is excluded.

| Input | Decode | Encode to fresh memory buffer | Output bytes |
|---|---:|---:|---:|
| 64×64 icon | 19.96 µs | 84.60 µs | 16,540 |
| 1024×1024 repeated-icon sheet | 4.82 ms | 21.74 ms | 4,196,508 |

The byte-preserving optimization pass reduced the 1024×1024 sheet writer to
about 4.0–4.5 ms (roughly 5× faster), and decode to about 3.2–3.5 ms. Byte-backed
images preserve the public pixel-slice API while letting the RGBA import path
reuse its inflated buffer. Three fresh-process 4096×4096 sheet decodes reduced
median peak RSS from 132,656 KiB to 67,156 KiB, matching the reference's roughly
67,000 KiB. Other input formats still require separate output storage.

See the [comparison record](../benchmarks/png/README.md) for raw baseline,
intermediate and final measurements, independent output checks, remaining
feature gaps and experiments that were not adopted. These observations come
from a shared Linux host and are not portable performance guarantees. Native
Linux/macOS/Windows CI checks correctness; timing is not a CI gate.

The first optional compression pass reduces the 1024 sheet from 4,196,508 to
52,549 bytes, compared with reference Fast's 53,405 and Balanced's 11,233.
Its measured encoding time is 3.35 ms versus Fast's 0.94 ms; this is size parity
on one workload, not general speed or feature parity. The final comparison adds
independent decoding of compressed output for all 31 profile fixtures and a
multi-block noise image. `png::encode` still matches all baseline byte hashes.

The next byte-preserving pass groups Adler32 into packed 128-byte sums, with
explicit no-carry bounds and no additional heap storage. In the recorded Linux
run, 1024 sheet compressed encoding fell from 3.35 to 2.21 ms and decode from
3.31 to 2.16 ms; reference Fast encoding/decode were about 0.92/0.89 ms.
The checksum microbenchmark fell from 1.75 to 0.55 ms per 4 MiB. Both writer
modes retain their previous exact SHA256 on all seven benchmark images. See the
comparison record for samples, methodology and remaining gaps, and the algorithm
notes for the recurrence derivation and creator acknowledgments.

A distance-one DEFLATE reference now fills the repeated byte directly after the
existing history, window, length, output-capacity and work checks. This reduces
1024 sheet decode to 1.68 ms in `run-fill.json`, versus 2.16 ms after the
checksum pass and 3.31 ms before it; the reference in that run measures 0.89 ms.
An exact independently constructed maximum-length distance-one stream is
checked by Python zlib and the Rust fixture test before the optimization.
Other distances retain the general overlap-copy path.


The latest encoder pass checks maximal distance-one runs before scanning the
hash candidate, computes distance-code ranges directly, and shortcuts length
258. A direct no-run branch retains literal-heavy throughput. Match selection,
dictionary updates, filters, budgets, allocations and both writers' bytes remain
unchanged. Exhaustive valid-distance and reference-bit tests preceded the
changes. The original reference corpus now measures 1024 sheet compressed
encoding at 1.597 ms versus reference Fast 0.816 ms (about 1.96× slower).
The generated paired suite is a separate workload, with approximately 30% less
sheet encoding time locally; shared-host results are observations. A bit-packing
experiment was rejected when alternating measurements showed no reliable gain.
