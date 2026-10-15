# Two-stream Adler: complete-codec decision

Retain candidate `17cc0ac`: two fixed interleaved streams with separate
reductions improve the original sheet and compressed encoding across Linux,
macOS and Windows. No universal speedup or general crate parity is claimed.
The [row-locality and arithmetic screening record](row-fusion-adler.md) retains
rejected trials; row-copy fusion was reverted before this experiment.

## Design and test evidence

`src/checksum.rs::Adler::update` retains the existing 128-byte groups, 65,535-byte
modulo interval, scalar tail and incremental interface. Each stream processes
eight eight-byte words. Reducing each stream independently avoids the packed
recombination chain in the earlier rejected interleaving trial. Production has
two fixed streams, without a generic tuning parameter, CPU dispatch, new heap
allocation, dependency, unsafe block or assembly. Four additional u64
accumulators add 32 bytes of logical temporary state. The measured benchmark
executable grows by 64 bytes on Linux and zero on macOS/Windows.

The [derivation and no-carry bounds](../algorithm-notes.md#two-interleaved-packed-adler-streams)
credit Mark Adler and L. Peter Deutsch/Jean-loup Gailly's RFC 1950. This is an
independent regrouping of the defining checksum, not code from a crate or paper.
Our x86-64 standalone release assembly still uses scalar word instructions in
Adler; no SIMD gain is asserted. Other architecture code generation was not
inspected for this trial.

Existing independent byte-weight, scalar checksum, full all-255 block and
fragmented-update tests passed before the refactor and after it. All **68** core
tests, formatting and all-target Clippy pass natively on all three platforms in
[CI 38796462226](https://github.com/SirElderling/raipixel/actions/runs/38796462226).
Local compiler is Rust 1.99.0; native stable is 1.99.1. Each paired comparison
uses the same compiler/settings for both libraries. Root manifests and lockfile
remain unchanged and dependency-free.

## Native complete-codec effect

Current / baseline `9122259`; that baseline has the retained RGBA filters and
restored two-loop row layout. Both generated and original suites isolate Adler.
Default release settings, fresh allocation, work charging and integrity checks
are included. Verification/I/O are untimed. Seven rotating rounds use 100 ms
calibrated batches and adjacent ABBA processes per case.

| Input / operation | Linux | macOS | Windows |
|---|---:|---:|---:|
| comparison-sheet-1024 / decode_input | 0.882x | 0.929x | 0.932x |
| comparison-sheet-1024 / decode_stored | 0.935x | 0.977x | 0.912x |
| comparison-sheet-1024 / encode_stored | 0.915x | 0.956x | 0.965x |
| comparison-sheet-1024 / encode_compressed | 0.877x | 0.874x | 0.874x |
| comparison-texture-1024 / decode_input | 0.868x | 0.938x | 0.930x |
| comparison-texture-1024 / decode_stored | 0.929x | 0.980x | 0.989x |
| comparison-texture-1024 / encode_stored | 0.922x | 0.983x | 0.979x |
| comparison-texture-1024 / encode_compressed | 0.902x | 0.904x | 0.909x |

Original-sheet decode takes **6.8-11.8% less time**; compressed encoding takes
**12.3-12.6% less**. Original-texture decode takes 6.2-13.2% less, compressed
encoding 9.1-9.8% less. Local original-sheet decode/encoding ratios are
0.908x/0.886x; texture 0.811x/0.853x. These local runs are separate observations,
not additive with native gains.

Regressions remain visible: Linux generated icon-16 decode measures 1.052x
(5.2% slower), while macOS/Windows measure 0.975x/0.969x and the local run
0.946x. macOS generated noise decode measures 1.021x, icon-16 compressed encode
1.017x; Windows generated-texture compressed encode 1.018x. Other generated
operations improve in these native runs. Retain the broad large-input benefit
and small fixed implementation; do not add size-specific dispatch to tune away
individual observations. Raw samples and ranges are retained for assessment.

All eight original-corpus artifacts and ten generated writer files agree
between baseline/current and across platforms. Original RGBA digests match the
independent corpus. Decoder flags, remaining work and both writers are exact.
The independent Pillow/public-reference consumer run preserves all fourteen
recorded original writer hashes, both writers on all 31 accepted-profile
fixtures, and multi-block noise. Sheet compressed sizes remain 52,549 bytes ours
and 53,405 reference Fast; transparent RGB is preserved.

## Current distance from the crate

Default release profile, same input and settings for our library and isolated
public-API `png =0.18.1`. No crate/dependency implementation source is read.
The full default/single-codegen-unit/thin-LTO matrices and all samples remain
recorded. Production settings stay unchanged.

| Input | Linux decode / ref | macOS decode / ref | Windows decode / ref | Linux encode / Fast | macOS encode / Fast | Windows encode / Fast |
|---|---:|---:|---:|---:|---:|---:|
| icon-64-compressed | 0.718x | 0.717x | 0.755x | 1.163x | 1.057x | 1.180x |
| sheet-1024-compressed | 1.149x | 1.149x | 0.995x | 1.173x | 0.761x | 1.019x |
| texture-1024-compressed | 1.117x | 1.225x | 1.003x | 0.197x | 0.174x | 0.149x |
| noise-256-compressed | 2.475x | 2.070x | 2.333x | 1.198x | 2.890x | 2.196x |
| comparison-sheet-1024 | 1.192x | 1.150x | 1.025x | 1.160x | 0.780x | 0.990x |
| comparison-texture-1024 | 0.380x | 1.005x | 0.544x | 0.198x | 0.177x | 0.146x |

Original-sheet decode remains **1.025-1.192x** reference: approximately 2.5-19.2%
more time, requiring a further 2.4-16.1% time reduction to equal this observation.
Original-sheet encoding reaches approximately Fast time on Windows, beats it on
macOS, and remains about 16% slower on Linux. This is not portable general
parity. Noise decode remains 2.07-2.47x reference, with large encode gaps on
macOS/Windows. Texture comparisons also reflect very different Fast output
sizes and do not establish Balanced-size or feature parity. 16-bit/APNG and
other documented feature gaps are unchanged.

Reference ratios from different workflow runs must not be treated as controlled
before/after effects: hosts, process timings and compiler patch versions can
change. Use the within-run paired baseline/current measurements above to
attribute gains to this change.

## Remaining stage costs

| Original sheet, microseconds | Plain decode | Inflate (nested Adler) | Reconstruct/adopt |
|---|---:|---:|---:|
| linux | 1122.5 | 537.2 (241.2) | 592.9 |
| macos | 1335.6 | 645.3 (210.9) | 651.2 |
| windows | 1549.6 | 1012.2 (207.9) | 464.4 |

Nested Adler is already included in inflate. Timers come from temporary
instrumented copies, whose plain/instrumented samples are retained; these are
estimates, not additive exact production costs. Reconstruction is a major
remaining original-sheet cost on Linux/macOS. Windows retains a larger
non-Adler inflate cost. Next candidates should isolate Paeth/row reconstruction
and Huffman/history expansion rather than repeat the rejected row-copy fusion.
Noise still requires separate chunk/CRC analysis; the mixed-filter sheet does
not stand in for every workload.

## Rejected smaller prior reduction

Each stream's eight prior byte columns total at most `8*28*255=57120`, so a
16-bit prefix multiply is mathematically sufficient instead of the existing
32-bit pair reduction. `tools/png_adler_prior_trial.py` makes this replacement
only in a temporary copy and reuses the stream probe's scalar verification and
nine rotating 25 ms batches. Duplicate baseline/candidate slots expose timing
variation. All checks pass, but baseline medians are 224.546/224.956 microseconds
versus candidate 444.591/435.983: about **1.94-1.98x** time. Reject it; shorter
source/arithmetic does not guarantee faster compiler output. Production retains
the tested pair reduction. Credit remains Adler, Gailly and Deutsch/RFC 1950;
no additional paper or implementation is adopted.

## Records and reproduction

- `native-adler-streams-{linux,macos,windows}.json`: both paired corpora and
  actual decoder stages, with run/compiler/source/input fingerprints.
- `build-profiles-adler-streams-*.json`: complete public-reference matrices.
- `timing-adler-streams-original.json`, `timing-adler-streams.json`,
  `decode-stages-adler-streams.json`, `run-adler-streams.json`: local results.
- `adler-prior-trial.json`: rejected reduction, source/script fingerprints,
  controls and every sample. The narrow prior variant never enters production.

Source fingerprints were checked against `17cc0ac`, allowing native Windows
CRLF checkout bytes. CI metadata names the synthetic PR merge revision; local
pre-publication runs can name the preceding HEAD with a dirty tree. Source
hashes identify the measured candidate in either case. The earlier standalone
dot/stream screens reproduce at `9122259`, their recorded baseline source.

```sh
cargo test --locked --offline
python tools/png_timings.py --baseline 9122259844497a57149dbe6ba7fa0ef708398c46 --corpus benchmarks/png/original-inputs.json --batch-ms 100 --output original-timings.json
python tools/compare_png.py --output independent.json
python tools/png_adler_prior_trial.py
```

