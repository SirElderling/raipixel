# Decoder profiling and wider CRC slicing

This pass follows [wide bit output and cached match prefixes](wide-buffer-and-prefix.md).
It changes only the shared CRC implementation, preserving every decoded sample,
encoded byte, error/checksum policy and charged work unit. No third-party source
was inspected or copied. Production still uses safe Rust and the standard
library only; no CPU-specific dispatch or assembly is introduced.

## Measure the complete decoder first

`tools/png_decode_stages.py` copies our source into a temporary instrumented
package and builds it beside the plain library under the same release profile.
The benchmark verifies identical pixels before timing. Each decode includes
fresh allocation and result destruction; file I/O and verification are outside
the measured batches. Nine alternating plain/instrumented batches are each
calibrated to at least 25 ms. Compiler, source and input hashes accompany samples.
CRC is nested within chunk processing; Adler is nested within inflate. Nested
timers must not be added to the parent times. Instrumentation changes code
layout and adds per-chunk clocks/atomics; the paired plain timings expose its
cost. Stage figures are estimates, not a claim of exact additive profiling.

The first native profile used unchanged production behavior at `ad47242`.
Median times below are microseconds; each stage column comes from the
instrumented build, while complete decode comes from the plain build.

| Platform / input | Plain decode | Chunks (CRC nested) | Inflate (Adler nested) | Reconstruct/adopt |
|---|---:|---:|---:|---:|
| Linux / noise-256 | 211.8 | 166.2 (150.3) | 31.0 (23.5) | 15.7 |
| Linux / sheet-1024 | 1048.0 | 29.2 (24.6) | 754.5 (378.6) | 264.8 |
| Linux / texture-1024 | 1406.9 | 56.0 (50.0) | 1050.0 (377.3) | 268.2 |
| macOS / noise-256 | 252.4 | 223.5 (208.5) | 30.4 (22.8) | 14.6 |
| macOS / sheet-1024 | 1334.4 | 45.0 (33.2) | 1035.4 (368.9) | 342.6 |
| macOS / texture-1024 | 1875.5 | 83.5 (67.1) | 1626.8 (398.2) | 351.4 |
| Windows / noise-256 | 260.5 | 237.4 (108.6) | 18.6 (14.0) | 12.1 |
| Windows / sheet-1024 | 1394.7 | 25.1 (18.0) | 1080.3 (225.5) | 149.1 |
| Windows / texture-1024 | 1574.5 | 42.0 (36.5) | 1223.6 (224.8) | 151.2 |

The primary noise cost is chunk processing and checksum validation, rather
than inflate. Windows also spends substantial chunk time outside the nested
CRC timer; allocation and IDAT joining are candidates for finer measurement,
not established causes. Sheets and textures spend most time in inflate,
including Adler32, so CRC alone cannot close all decoder gaps.

## Test-first CRC screen

Before changing the arithmetic, a new regression test checks CRC against the
independent bitwise PNG polynomial recurrence at every length through 129,
several larger boundaries through 65,537, empty parts and splits through 65.
The final candidate also checks 4,095/4,096/4,097-byte crossover cases.
The existing standard vector, fragmented-input, decoder corruption and exact
writer tests remain. These behavior-preserving tests pass on both the existing
correct implementation and the candidate; there is no claimed functional red
failure for a speed-only change.

`tools/png_crc_trial.py` freezes our original checksum source at `09c7359`,
then independently generates 16- and 32-byte slicing versions. All three must
agree with the bitwise oracle before nine rotating calibrated timing batches.
No reference-crate implementation is consulted. For 256 KiB inputs:

| Platform | 8-byte µs | 16-byte µs | 32-byte µs | 32 / 8 |
|---|---:|---:|---:|---:|
| Linux | 147.60 | 119.89 | 73.08 | 0.495 |
| macOS | 197.03 | 108.59 | 108.29 | 0.550 |
| Windows | 105.73 | 83.60 | 56.67 | 0.536 |

32-byte slicing wins clearly on x86_64 and is close to 16-byte slicing on ARM64.
The initial version adds an eight-byte remainder loop. The final candidate
also keeps payloads smaller than 4 KiB entirely on the original eight-byte loop
to limit the table working set for compact compressed IDAT chunks. It extends the same polynomial-generated tables from
8 to 32 KiB: **24 KiB more static read-only data**, no new heap storage, and no
runtime initialization. Explicit table contributions keep the hot loop simple.
The independent mathematical derivation and acknowledgment of Abhijeet
Joglekar, Michael E. Kounavis and Frank L. Berry's FAST 2005 paper are in
[algorithm notes](../algorithm-notes.md#wider-crc-slicing-for-stored-png-payloads).
The paper's CRC32c polynomial is not used; PNG's polynomial stays unchanged.
16-byte slicing is retained as a measured alternative in the experiment only.

## Complete-codec acceptance

Native CI builds the identical complete-codec driver against `ad47242` and the
initial candidate `dffd648`, followed by longer measurements of the same
production code at `b16c08f` and the payload-size candidate `c8334f1`, with ABBA
process order per case. This isolates the CRC
change from the previous encoder pass. The suite checks exact SHA256 equality
for every stored/compressed output before timing. All correctness and Clippy
checks pass natively. Core test count is now **66**: 22 raster, 19 JSON, ten PNG,
ten DEFLATE and five checksum tests.

Initial 25 ms run (current / baseline time; smaller is faster):

| Input / operation | Linux | macOS | Windows |
|---|---:|---:|---:|
| noise-256 / decode_input | 0.622× | 0.645× | 0.782× |
| sheet-1024 / decode_input | 1.009× | 1.099× | 0.993× |
| sheet-1024 / encode_compressed | 1.001× | 1.163× | 0.996× |
| texture-1024 / decode_input | 1.008× | 0.871× | 0.990× |
| texture-1024 / encode_compressed | 0.969× | 1.016× | 1.013× |
| sheet-1024 / decode_stored | 0.584× | 0.740× | 0.774× |
| sheet-1024 / encode_stored | 0.574× | 0.709× | 0.717× |

The initial macOS sheet regression coincided with large per-batch stalls, so
we repeated the complete-codec experiment with a 100 ms minimum batch at
`b16c08f` (same production source). The sheet regression did not repeat on
macOS, but other compact cases regressed, including macOS icons and Windows
sheets. Longer batches did not establish uniform gains; these results are
retained rather than treated as proof that all regressions were noise.

| Input | Linux decode | macOS decode | Windows decode | Linux encode | macOS encode | Windows encode |
|---|---:|---:|---:|---:|---:|---:|
| icon-16 | 1.022× | 0.986× | 0.971× | 0.993× | 1.076× | 1.001× |
| icon-64 | 1.001× | 1.195× | 1.038× | 0.988× | 1.227× | 1.105× |
| noise-256 | 0.641× | 0.676× | 0.661× | 0.933× | 0.934× | 0.921× |
| sheet-1024 | 1.003× | 1.029× | 1.099× | 0.996× | 0.993× | 1.091× |
| texture-1024 | 1.013× | 0.996× | 1.031× | 0.983× | 0.918× | 1.016× |


The final candidate uses the original eight-byte loop below 4 KiB and the
32-byte loop for larger payloads. This keeps the smaller table working set on
the measured small compressed chunks. It is a simple size policy, not a new
format constraint or a claim that the crossover is universally optimal.

The final 100 ms native run passes all checks on Linux, macOS ARM64 and
Windows MSVC. Each ratio below is current / `ad47242`; encode means the complete
compressed writer:

| Input | Linux decode | macOS decode | Windows decode | Linux encode | macOS encode | Windows encode |
|---|---:|---:|---:|---:|---:|---:|
| icon-16 | 1.043× | 1.038× | 0.978× | 0.973× | 1.066× | 1.006× |
| icon-64 | 1.036× | 0.963× | 0.979× | 0.976× | 0.950× | 1.016× |
| noise-256 | 0.639× | 0.640× | 0.602× | 0.921× | 0.880× | 0.903× |
| sheet-1024 | 0.998× | 0.715× | 1.001× | 0.972× | 0.816× | 1.001× |
| texture-1024 | 1.020× | 0.930× | 1.002× | 0.934× | 0.903× | 1.040× |

| Large stored encoding | Linux | macOS | Windows |
|---|---:|---:|---:|
| noise-256 | 0.611× | 0.627× | 0.642× |
| sheet-1024 | 0.593× | 0.572× | 0.724× |
| texture-1024 | 0.604× | 0.640× | 0.759× |

Noise decoding takes **36–40% less time** and noise compressed encoding
8–12% less time. Large stored encoding takes **24–43% less time**. Small cases
remain mixed: icon-16 decode is about 4% slower on Linux/macOS, macOS icon-16
encoding about 7% slower, and Windows texture encoding about 4% slower. The
large macOS sheet gain is not assumed to come entirely from CRC: that case
mostly takes the original small-payload path, and native runs show substantial
variance. We retain the consistent large-payload benefit, record every
regression, and make no universal speedup claim.

The benchmark executable grows by 25,256/16,512/25,088 bytes on
Linux/macOS/Windows; static tables grow by exactly 24 KiB. No image/dictionary
heap allocation or work-budget policy changes.

Decode / reference decode, with both libraries under the default release profile:

| Input | Linux | macOS | Windows |
|---|---:|---:|---:|
| icon-64 | 0.795× | 0.815× | 0.858× |
| sheet-1024 | 1.399× | 1.247× | 1.183× |
| texture-1024 | 1.297× | 1.280× | 1.159× |
| noise-256 | 3.347× | 2.364× | 1.408× |

Compressed encode / reference Fast, with both libraries under the default release profile:

| Input | Linux | macOS | Windows |
|---|---:|---:|---:|
| icon-64 | 1.103× | 1.161× | 1.135× |
| sheet-1024 | 1.138× | 0.825× | 1.166× |
| texture-1024 | 0.218× | 0.176× | 0.170× |
| noise-256 | 1.265× | 3.069× | 1.794× |

We remain behind in several cases. Noise decode still takes 1.41–3.35× the
reference time in this run; generated-sheet decode 1.18–1.40×. The single macOS
sheet-encoding win does not establish general parity. The reference Fast
texture output is much larger, so its timing is not a Balanced-size comparison.
Default build settings remain unchanged; single-unit/thin-LTO experiments and
all raw samples remain available.

Local [unrestricted-width paired results](../timing-crc32.json) measure noise input decoding at
0.607× baseline time, stored decoding at 0.591× and stored encoding at 0.594×.
Compressed-sheet decode/encode measure 1.021×/1.023×; texture decode is 1.000×.
Those mixed local results remain visible. Timings are machine/workload
observations; native paired comparisons carry the acceptance decision.

The [local final-candidate paired report](../timing-crc-hybrid.json) measures
noise decode at 0.665× baseline time, sheet decode 1.064×, sheet compressed
encoding 1.025× and texture compressed encoding 0.897×. These mixed local
results are also retained. The final [actual-stage profile](../decode-stages-hybrid.json)
uses the exact published candidate source.

Local unrestricted actual-stage profiles ([before](../decode-stages-before.json),
[after](../decode-stages-after.json)) and the [width screen](../crc-trial.json)
retain every raw sample. The final paired report fingerprints the exact
published candidate source. The first profile predates the new regression test;
test-only differences do not alter its release behavior.

The [final independent original-corpus run](../run-crc-hybrid.json) preserves all fourteen
recorded writer hashes, checks all 31 supported profile fixtures against
independent expectations/readers, and verifies multi-block noise. Compressed
sheet output remains 52,549 bytes versus reference Fast's 53,405, while this
run's encoding times are 1.107/0.782 ms. This is a different workload from the
generated native sheet; changes between separate host runs are not treated as
causal performance gains. No additional PNG features are implemented here.

## Follow-up priorities

The [actual-stage profile of the original corpus](../decode-stages-original.json)
changes the priority for the original sheet: its plain decode is 1,458 µs,
with instrumented row reconstruction/adoption about 803 µs and inflate about
684 µs (375 µs nested Adler). This sheet has 896 Up, 80 Sub and 48 Paeth scanlines, unlike our own
Sub/Up-generated input. Counts come from independently inflating the checked
input with Python stdlib zlib and reading each RGBA8 scanline filter byte. The original texture is about 926 µs plain decode,
with 732 µs inflate (341 µs nested Adler) and 186 µs reconstruction. These
local input-specific estimates do not establish the reference's internal costs.
Measure filter reconstruction for the original sheet and inflate/Adler for
textures before choosing the next implementation change.
For noise, measure the remaining chunk processing separately, particularly
IDAT joining/allocation on Windows, before changing the decoder architecture.
The wider table has a real cache/binary footprint cost; retain the complete
codec measurements when evaluating further checksum changes. Reaching parity
on one sample does not establish general output, feature or performance parity.


## Reproduction and retained records

- Baseline native profile/width screen: [CI 38775193387](https://github.com/SirElderling/raipixel/actions/runs/38775193387), `ad47242`.
- Unrestricted slicing, 25 ms: [CI 38775549514](https://github.com/SirElderling/raipixel/actions/runs/38775549514), `dffd648`.
- Unrestricted slicing, 100 ms: [CI 38776160250](https://github.com/SirElderling/raipixel/actions/runs/38776160250), `b16c08f` (same production source).
- Final size policy, 100 ms: [CI 38777150052](https://github.com/SirElderling/raipixel/actions/runs/38777150052), `c8334f1`.

The raw `native-crc32-initial-{linux,macos,windows}.json` records contain the
before stages, width screen, first paired run and after stages. The
`native-crc32-repeat-*.json` records retain the longer unrestricted run;
`native-crc-hybrid-*.json` contain the final paired/stage measurements.
Corresponding `build-profiles-crc32-initial-*`, `build-profiles-crc32-repeat-*`
and `build-profiles-crc-hybrid-*` files retain every matched reference/profile
sample. Source fingerprints are verified against their Git revisions, allowing
for native Windows CRLF checkout bytes. All ten suite writer hashes also match
across the three native platforms. No profile-only or favorable build ratio is
used as a production parity claim.

```sh
python tools/png_crc_trial.py
cargo run --release --locked --offline --example png_bench -- --export png-inputs
python tools/png_decode_stages.py png-inputs/noise-256-compressed.png png-inputs/sheet-1024-compressed.png png-inputs/texture-1024-compressed.png
python tools/png_timings.py --baseline ad47242649c004b0d22273aa2c87b543ef1d80e8 --batch-ms 100 --output timings.json
python tools/png_build_profiles.py --output profiles.json png-inputs/icon-64-compressed.png png-inputs/sheet-1024-compressed.png png-inputs/texture-1024-compressed.png png-inputs/noise-256-compressed.png
```
