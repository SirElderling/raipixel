# RGBA filter reconstruction

The [decoder profile](decoder-crc.md) identified row reconstruction as the
largest local cost on the original sheet: about 803 µs versus 684 µs inflate
within a roughly 1.46 ms plain decode. That input has 896 Up, 80 Sub and 48
Paeth scanlines. Our generated native sheet instead uses Sub/Up throughout.
This pass targets the actual mixed-filter input rather than assuming all
decoders have the same dominant stage.

## Small, typed pixel loops

`src/png.rs::sub_rgba` carries four decoded-left channel states across each row.
`paeth_rgba` carries decoded-left and previous-upper-left pixels and applies
our existing predictor independently to each channel. Using `[[u8; 4]]` views
exposes the four channel recurrences without a backwards byte index and
per-byte neighbor-boundary decisions.

The guarded path applies only when bytes per pixel is four. In the supported
profile this is RGBA8, whose row length is always a multiple of four, including
each Adam7 pass. First-row Paeth reduces to Sub when all upper neighbors are
zero. Other color layouts retain the existing generic loops. No heap buffer,
format feature, work charge, resource check, filter-selection or writer changes.

The recurrence proof and source acknowledgments are in
[algorithm notes](../algorithm-notes.md#rgba-sub-recurrence). We credit the PNG
Working Group's normative filter rules and Alan W. Paeth's *Image File
Compression Made Easy*, *Graphics Gems II* (1991), as identified by PNG section
9.4. The implementation uses the PNG adaptation; no chapter or reference-crate
source was copied. The pixel-state grouping is independently derived.

## Test-first correctness and corpus measurements

Each new test was run before its helper existed and failed compilation. The Sub
test checks byte-by-byte reconstruction across short and long supported row
widths and varied samples. The Paeth test uses independently expressed neighbor
distances, representative low/high byte combinations, tie priority, consecutive
rows, helper/routing agreement and first-row behavior. Existing independent
fixtures cover all supported colors/depths, Adam7 and malformed inputs.

All **68** core tests pass: 22 raster, 19 JSON, ten PNG integration, two PNG
filter unit, ten DEFLATE and five checksum tests. Formatting and all-target
Clippy pass natively on Linux x86_64, macOS ARM64 and Windows MSVC.

The timing runner now accepts
`--corpus benchmarks/png/original-inputs.json`. This preserves the exact two
original PNG byte streams and independently verified decoded RGBA SHA256
values, with their Pillow provenance. CI requires only Python/Rust standard
libraries. Before timing, each version exports decoded samples, flags,
remaining work and both writer outputs. Every artifact must agree between
versions, and RGBA bytes must also match the independent expected digest.
This prevents accepting a speedup that changes pixels or silently omits work.

The existing driver still measures complete decoding, stored decoding and both
writers with fresh allocation, calibrated 100 ms minimum batches, seven
rotating rounds and adjacent ABBA processes per image. File I/O, setup,
verification and corpus extraction are untimed. Both libraries use the same
driver/compiler/profile; hashes and every sample are retained.

## Native paired results

All ratios below are current / pre-filter baseline `b77a1e8`; smaller is
faster. Sub alone, code `52ffac2`, CI
[38779058520](https://github.com/SirElderling/raipixel/actions/runs/38779058520):

| Input decode | Linux | macOS | Windows |
|---|---:|---:|---:|
| comparison-sheet-1024 | 0.909× | 0.905× | 0.923× |
| comparison-texture-1024 | 1.000× | 1.081× | 1.011× |

This establishes a 7.7–9.5% observed original-sheet decode reduction. macOS
texture decode measures 8.1% slower in this first run; the raw result remains.

Sub plus Paeth, code `09cae7b`, CI
[38779418389](https://github.com/SirElderling/raipixel/actions/runs/38779418389):

| Input / operation | Linux | macOS | Windows |
|---|---:|---:|---:|
| comparison-sheet-1024 / decode_input | 0.804× | 0.791× | 0.826× |
| comparison-sheet-1024 / decode_stored | 1.001× | 1.010× | 1.003× |
| comparison-sheet-1024 / encode_stored | 1.016× | 1.046× | 1.003× |
| comparison-sheet-1024 / encode_compressed | 1.015× | 1.012× | 0.982× |
| comparison-texture-1024 / decode_input | 0.988× | 1.035× | 1.008× |
| comparison-texture-1024 / decode_stored | 1.003× | 0.959× | 0.990× |
| comparison-texture-1024 / encode_stored | 0.999× | 1.002× | 1.032× |
| comparison-texture-1024 / encode_compressed | 0.984× | 1.039× | 0.959× |

Original-sheet decode takes **17–21% less time** across the native platforms.
Texture is about 1.2% faster on Linux, 3.5% slower on macOS and 0.8% slower on
Windows. Writers remain unchanged; their small timing movements are controls,
not filter speedups. Stored encoding of the original sheet measures 4.6%
slower on macOS in this run. We retain the mixed observations and do not claim
uniform gains.

Benchmark executable growth versus the same driver/pre-filter library is
80 bytes on Linux, zero on macOS and 512 bytes on Windows. No table or heap
allocation is added. Original corpus samples, flags, work charges and both
writer hashes agree across versions and native platforms.

## Distance from the reference

The matched whole-codec matrix now includes the exact original PNGs alongside
the generated inputs. Both libraries receive default, single-codegen-unit and
thin-LTO settings in adjacent mirrored order. The default profile remains the
production setting. Current/default decode ratios against public-API
`png =0.18.1`:

| Input | Linux decode/reference | macOS decode/reference | Windows decode/reference |
|---|---:|---:|---:|
| icon-64-compressed | 0.830× | 0.784× | 0.836× |
| sheet-1024-compressed | 1.416× | 1.243× | 1.138× |
| texture-1024-compressed | 1.306× | 1.188× | 1.150× |
| noise-256-compressed | 3.136× | 2.488× | 1.443× |
| comparison-sheet-1024 | 1.386× | 1.257× | 1.165× |
| comparison-texture-1024 | 0.452× | 1.019× | 0.590× |

The original sheet still takes **1.17–1.39×** reference decode time. That is a
remaining gap, not parity. The original texture differs substantially from
our own encoded texture; a speedup on one input is not a feature/conformance
or general performance-parity claim. No reference implementation source was
inspected.

The final [independent corpus run](../run-sub-paeth.json) preserves all fourteen
recorded writer hashes and checks both writers on all 31 supported profile
fixtures plus multi-block noise. It measures original-sheet decode at
1.274/0.905 ms ours/reference, and compressed encode at 1.126/0.833 ms ours/Fast.
Compressed sizes remain 52,549/53,405 bytes. These shared-host observations are
separate from the controlled native before/after results.

## What remains hot

Current actual-stage estimates for the original sheet:

| Original sheet, µs | Plain decode | Inflate (nested Adler) | Reconstruct/adopt |
|---|---:|---:|---:|
| Linux | 1056.9 | 576.2 (330.1) | 484.8 |
| macOS | 1517.5 | 749.0 (309.1) | 656.3 |
| Windows | 1939.1 | 1247.2 (367.4) | 588.7 |

Nested Adler is included in inflate and must not be added again. Plain and
instrumented totals are both retained; instrumentation changes code layout
and adds timers, so stage values are estimates. On Linux, inflate is about
576 µs with 330 µs nested Adler, and reconstruction about 485 µs. Both remain
useful targets; checksum arithmetic and remaining filter/row work need separate
experiments. Up-heavy generated inputs do not establish the mixed-filter
sheet's bottleneck.

The [local Sub profile](../decode-stages-sub.json), [combined profile](../decode-stages-sub-paeth.json),
[Sub paired run](../timing-sub-original.json) and
[combined paired run](../timing-sub-paeth-original.json) are retained. Local
paired sheet decode is 0.907× for Sub and 0.811× for the combined version,
each against the same pre-filter baseline. These are separate runs, not
additive stage savings.

## Rejected narrowing probe

The [16-bit Paeth trial](../timing-paeth-i16.json) is correct on the filter
oracles: all predictor intermediates fit within -510..510. It measures sheet
decode at 0.915× the combined 32-bit baseline, but also measures improvements
in unchanged writers and the Up-heavy texture. This does not isolate a gain
from narrowing the predictor. We keep the simpler existing 32-bit arithmetic
and record the [one-line rejected patch](paeth-i16.patch). It was not retained
in production or used for the reported native performance claims.

## Records and reproduction

`native-rgba-sub-{linux,macos,windows}.json` retain the first original-corpus
paired runs and stages. `native-rgba-filters-*.json` retain the final original,
generated and actual-stage results. The older generated suite still compares
against `ad47242` and includes the earlier CRC change; it is not used to claim
filter-only gains. The original-corpus runs compare against `b77a1e8` and
isolate this pass.

Corresponding `build-profiles-rgba-sub-*` and
`build-profiles-rgba-filters-*` files retain the complete reference/profile
matrices. Library and input fingerprints are verified against Git and the
fixed corpus, allowing Windows CRLF checkout bytes. The public crate remains
isolated from the production dependency graph.

```sh
cargo test --locked --offline
python tools/png_timings.py --baseline b77a1e8eaccc1afb6a240eafd50f2aa7c2e2d360 --corpus benchmarks/png/original-inputs.json --batch-ms 100 --output original-timings.json
```
