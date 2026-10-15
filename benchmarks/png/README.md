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

Feature gaps remain substantial: raipixel writes only RGBA8 stored data, has no
streaming-row decode, and explicitly rejects 16-bit/APNG. Reference 16-bit and
two-frame APNG read/write were exercised through public APIs. Metadata parsing,
additional output sample formats and selectable compression are also outside
our current implementation. Pixel equality does not establish robustness parity.

`tools/png_compare.rs` records the independent comparison harness. It requires
an isolated Cargo package with raipixel as a local path dependency and
`png = "=0.18.1"`; it must not be added to the production manifest. The optional
runner sets up that isolated environment and records subsequent measurements:

```sh
python tools/compare_png.py --output benchmarks/png/optimized.json
```

This verification facility requires Pillow and network access for uncached
reference dependencies. Production builds and std-only Rust tests remain offline
and dependency-free. The reference lockfile pins the comparison environment.

Initial speed optimizations must retain exact PNG bytes. The regression corpus
`tests/fixtures/writer-*.png` freezes rows and stored-block boundaries;
SHA256 values in baseline measurements support larger output comparisons.
Later lossless compression settings may change encoded bytes but must retain
all decoded RGBA samples, including transparent RGB. Keep that a separate,
explicit comparison from byte-preserving throughput changes.
