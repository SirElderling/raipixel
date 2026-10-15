# PNG comparison measurements

`baseline.json` records raipixel at commit 044f5e5 against the public API of
`png` 0.18.1 with default Cargo features. No reference implementation source
was inspected or copied. The reference was compiled in a separate temporary
Cargo package; it is not a raipixel dependency or a required test tool.

The measurement order and units are recorded in the JSON. All timing data
includes allocation, checksum work, encoding into real memory buffers and
observing the result through `black_box`. Input/output file I/O is excluded.
Seven batches rotate operation order; reported times are medians. Inputs are
an original transparent pixel icon, repeated sheets and deterministic RGBA
textures encoded independently. Reference decoding enables integrity checks,
ignores text/ICC payloads and expands supported static samples to RGBA8.

| 1024×1024 sheet | raipixel baseline | png 0.18.1 |
|---|---:|---:|
| Decode | 4.82 ms | 0.85 ms |
| Encode, stored/no compression | 21.74 ms | 0.43 ms |
| Encode, Fast | Not implemented | 0.80 ms |
| Encode, Balanced | Not implemented | 9.23 ms |
| Stored output | 4,196,508 bytes | 4,195,716 bytes |
| Fast output | Not implemented | 53,405 bytes |
| Balanced output | Not implemented | 11,233 bytes |

A 4096×4096 sheet decode used median process peak RSS of 132,656 KiB for
raipixel and 67,072 KiB for the reference, measured in three fresh Linux
processes each. These are workload/machine observations, not portable limits.
CRC32 alone took about 10 ms per 4 MiB in a separate probe using our own
checksum function; Adler32 took about 1.2 ms. This identifies checksum work as
a concrete optimization target without attributing all remaining time to it.

Correctness: all 31 accepted profile fixtures match our independently supplied
sample expectations and are read exactly after re-encoding by an external
image consumer. The reference agrees on 30; its sub-byte tRNS fixture with
unused high bits yields opaque alpha while ours masks those bits and yields
transparent alpha. The masking rule is explicit in PNG Third Edition 11.3.1.1.
This is an isolated standards-backed disagreement, not evidence of general
conformance superiority. The JSON records rejected-profile inputs accepted by
the reference; some are deliberate policy differences such as bytes after IEND.

At the baseline, raipixel wrote only RGBA8 stored data, had no
streaming-row decode, and explicitly rejected 16-bit/APNG. Reference 16-bit and
two-frame APNG read/write were exercised through public APIs. Metadata parsing,
additional output sample formats and selectable compression are also outside
our current implementation. Pixel equality does not establish robustness parity.

`tools/png_compare.rs` records the independent comparison harness. It requires
an isolated Cargo package with raipixel as a local path dependency and
`png = "=0.18.1"`; it must not be added to the production manifest. The optional
runner sets up that isolated environment and records subsequent measurements:

```sh
python tools/compare_png.py --memory --output benchmarks/png/optimized.json
```

`--memory` is Linux-only and measures three fresh processes per decoder. A
small separate parent avoids inheriting the image generator's resident memory
before exec. Omit that flag for the portable pixel/timing comparison.

This verification facility requires Pillow and network access for uncached
reference dependencies. Production builds and std-only Rust tests remain offline
and dependency-free. The reference lockfile pins the comparison environment.

Initial speed optimizations must retain exact PNG bytes. The regression corpus
`tests/fixtures/writer-*.png` freezes rows and stored-block boundaries;
SHA256 values in baseline measurements support larger output comparisons.
Later lossless compression settings may change encoded bytes but must retain
all decoded RGBA samples, including transparent RGB. Keep that a separate,
explicit comparison from byte-preserving throughput changes.

## Byte-preserving optimization pass

The implementation changes are recorded in `e423565` (bulk checksums and row
copies) and `0955b39` (byte-backed images and adoption of decoded RGBA storage).
The public `Image::pixels()` view remains `&[[u8; 4]]`. Both changes use safe
stable Rust, introduce no dependencies and retain the limits and checksum
validation. No competing implementation source was inspected or copied.

- `bulk.json`: first checkpoint after slicing-by-eight CRC32, weighted Adler32
  sums and contiguous stored-row copies.
- `reused-rgba.json`: checkpoint after noninterlaced RGBA decoding reused its
  inflated allocation. `reused-rgba-memory.json` records the corresponding
  three-run memory check.
- `optimized.json`: repeat of the complete comparison, including automated
  fresh-process memory measurement and exact SHA256 checks against baseline.

| Workload / metric | Baseline | Optimized (observed repeats) | Reference (observed repeats) |
|---|---:|---:|---:|
| 1024×1024 sheet encode, stored | 21.74 ms | 4.0–4.5 ms | 0.4–2.3 ms |
| 1024×1024 sheet decode | 4.82 ms | 3.2–3.5 ms | 0.83–0.92 ms |
| 1024×1024 texture decode | 3.86 ms | 2.15–2.29 ms | 1.97–2.18 ms |
| 4096×4096 RGBA sheet peak RSS | 132,656 KiB | About 67,000 KiB | About 67,000 KiB |

The writer is roughly five times faster than our baseline. The RGBA sheet
memory gap is closed on this workload; this does not establish memory parity
for Adam7 or other color formats. Texture decode is close to the reference;
repetitive-sheet decode remains about four times slower. Reference stored
encoding varied significantly across processes, so a single relative speedup
against it would be misleading. The JSON retains the observations.

All seven benchmark outputs retain the exact baseline SHA256, and all 31
accepted fixtures retain the independently expected samples, including hidden
RGB. Full writer-byte regression fixtures and an import/edit test protect both
serialization and the public image behavior. The checksum tests also compare
bulk/fragmented calculations with the defining scalar recurrence, including
full accumulator blocks. That checkpoint had 54 std-only Rust tests.

Additional experiments were deliberately not adopted: grouping RGBA Paeth
reconstruction into four-byte lanes took roughly 0.82 ms versus 0.68 ms for
reconstruction in an isolated sheet probe; alternative 32-byte weighted and
16-lane Adler loops offered only about 6–15% in that checksum probe and added
complexity. These are exploratory own-code measurements, not portable claims.
The probe included allocation for reconstruction and excluded file I/O; the
complete comparison above determines the retained implementation's result.

In the byte-preserving phase, output size and feature coverage were unchanged.
The 1024 sheet took
4,196,508 bytes versus 53,405 with reference Fast and 11,233 with Balanced.
Reaching those sizes requires a separately tested lossless compressor/filter
policy and necessarily changes PNG file bytes; it must preserve decoded RGBA.
Streaming rows, 16-bit samples, APNG and metadata remain explicit feature gaps.
This pass improves throughput and memory without claiming full parity or
trading output correctness for benchmark values.


## Optional lossless compression and research credits

The new `png::encode_compressed` keeps every RGBA sample while changing the
encoded PNG bytes. The original `png::encode` retains its exact byte contract.
This is one explicit optional function, not a backend/settings framework.
See [algorithm notes and creator acknowledgments](algorithm-notes.md) for the
papers reviewed, ideas adopted partially, simplifications and deferred work.
PR1 makes those acknowledgments mandatory in its research guidelines.

The small implementation uses Sub/Up filtering, bounded greedy hash matching,
fixed Huffman codes, a distance-one run candidate and stored fallback. There
are no match chains, dynamic code construction or whole-image scratch buffers.
The dictionary resets per block, deliberately trading some ratio for simplicity.

`compressed-initial.json` records the first one-candidate matcher;
`compressed.json` adds the bounded run candidate. For the 1024 sheet this
reduced output from 72,224 to 52,549 bytes. An alternative that inserted the
end of every match into the table was rejected: it displaced useful entries
and failed the repetitive-image compression regression. The retained change
preserves the useful hash candidate and checks a run separately.

| Input | Our compressed bytes | Reference Fast bytes | Reference Balanced bytes | Our compressed time | Reference Fast time |
|---|---:|---:|---:|---:|---:|
| 16×16 icon | 228 | 279 | 198 | 3.14 µs | 1.72 µs |
| 64×64 icon | 479 | 470 | 301 | 14.30 µs | 5.78 µs |
| 256×256 sheet | 3,908 | 3,911 | 1,513 | 0.20 ms | 0.06 ms |
| 1024×1024 sheet | 52,549 | 53,405 | 11,233 | 3.35 ms | 0.94 ms |
| 1024×1024 texture | 83,186 | 3,316,027 | 12,533 | 3.10 ms | 5.55 ms |

This reaches approximately Fast-mode output size on the tested icon/sheets;
it does not reach Fast speed or Balanced size generally. The texture's different
result demonstrates how filter policies/content affect comparisons, not broad
superiority. Our tiny fixed-code writer has neither the feature coverage nor
compression choices of the reference. Times remain shared-host observations.

All seven original stored output hashes still match baseline. Both reference
and independent image decoders confirm exact compressed RGBA. The runner also
checks both writers on all 31 accepted profile fixtures and independently reads
a 257×257 pseudorandom RGBA image to exercise multi-block stored fallback.
56 std-only tests cover the new writer, independent fixed-code canonical bytes,
repeatability, block boundaries, compression of repeated data, noise fallback,
work preflight and I/O failure. Native CI validates Linux, macOS and Windows.
