# Matching and Adler32 bottleneck experiments

Baseline: 126a5568c32ef0e25d4281fed4580d0b6592da3a. Original implementation,
without inspection or copying of the comparison crate's source. Test-first
checkpoint ba249ae adds scalar expectations over overlapping distances and all
legal match caps/termination offsets; there are now **62 core Rust tests**.

## Match scan

The original loop compares eight-byte slices by indexing into the complete
block at each step. Experiments take two bounded views once, then compare
8/16/32-byte groups. Window/dictionary checks and the first-three-byte rejection
remain unchanged; both views have exactly `maximum-3` bytes.

[Initial width screen](../bottleneck-scan-screen.json) uses nine rotating batches,
calibrated to at least 25 ms, includes fresh encoder/output allocation and first
checks complete DEFLATE byte equality. The original filtered sheet takes
0.757 ms versus 0.613/0.624/0.641 ms for 8/16/32-byte views. Texture is
0.917/0.812/0.797/0.813 ms. The 16-byte option keeps noise close to baseline;
none of these isolated results alone establishes complete-writer performance.

The first [native whole-codec trial](https://github.com/SirElderling/raipixel/actions/runs/38751170672)
reduces sheet/texture encoding across all platforms, but noise rises about 5%
on Linux/Windows and 16% on macOS. Its source uses the first bounded scanner and
two Adler streams. Reports: [Linux](../native-match-adler-linux.json),
[macOS](../native-match-adler-macos.json), [Windows](../native-match-adler-windows.json).
These regressions are retained in the record, not hidden by the final trial.

We tested a [unified remainder](../match-remainder-screen.json),
[separate comparison helper](../match-split-screen.json) and
[explicitly inline rejection guard](../match-hot-prefix-screen.json). The
retained version keeps only candidate rejection inline and the larger
`matching_tail` body separate. The helper compares complete 16-byte groups,
then one scalar remainder; it does not change which match is chosen. This
avoids making every rejected hash candidate enter the larger scan. No target
architecture branch, unsafe operation, larger dictionary or input-sized index
is introduced. Exhaustive valid-domain/scalar tests check overlapping slices
and the existing full writer hashes check actual token/output stability.

The last stage screen gives noise 1.850 → 1.599 ms, sheet 0.805 → 0.687 ms and
texture 0.882 → 0.764 ms versus the original matcher in the same executable.
The earlier remainder/hint variants were not retained merely for small deltas.
The two inline attributes express the measured fast/slow split; neither helper
changes public API or allocation policy.

## Adler reduction

We first [interleaved two/four prefix streams](../adler-interleave-screen.json)
to shorten serial dependencies. The first local screen improved two-stream
Adler throughput, but the native isolated checksum results were inconsistent
and the four-stream version did not justify its extra state.

The next [reduction screen](../adler-reduction-screen.json) collects the same
packed-lane recurrence algebraically. The two-stream/direct-reduction version
measures about 0.377/0.364 ms on the two 4 MiB inputs, versus the two-stream
old reduction's 0.456/0.457 ms. The **serial** prefix/direct-reduction version
is about 0.386/0.383 ms: close enough to keep the simpler original prefix loop.
The retained code does not retain the extra interleaved accumulator arrays.

The [derivation and no-carry bounds](../algorithm-notes.md#bounded-match-views-and-direct-packed-lane-reduction)
show why the wrapping lane-sum multiplication and wider prior-pair reduction
preserve every Adler32 value. Existing tests isolate each byte weight, compare
with the scalar defining recurrence, fill complete blocks with 255 and vary
fragmentation around group/block boundaries. No checksum validation is weakened.

## Local whole-codec trial and limitations

[First local paired run](../timing-match-adler.json) and
[final local paired run](../timing-match-adler-final.json) retain all exact
suite writer hashes. Both use adjacent ABBA processes per case, the same driver,
compiler/release profile and seven rotating batches per operation. Full output
allocation, checksums and work charging are included; I/O/verification is not.
The final local run's compressed-encoding ratios (current/baseline) are 0.926
icon-16, 0.840 icon-64, 0.910 noise, 0.656 sheet and 0.784 texture. These are
shared-host observations; native results and unchanged CRC/other measurements
must be assessed before claiming a portable gain. The benchmark executable grows
608,008 → 608,616 bytes, **608 bytes** in that local build.

Adler32 also runs during decoding and original stored encoding, so those
operations are affected too; they are no longer unchanged controls for this
experiment. No wire-format capability or compression ratio is changed.

`tools/png_bottleneck_compare.py --baseline 126a556 INPUT.png ...` compiles our
old/current encoder and checksum modules into one executable, checks full
DEFLATE byte equality and scalar Adler32 before timing, and rotates nine
calibrated batches. Native CI runs it on generated sheet, texture and noise.
The [initial local stage comparison](../bottleneck-stage-comparison.json) belongs
to the first two-stream trial, not the final direct-reduction source. Source
fingerprints distinguish the revisions.

## Attribution

The matcher remains independently implemented around L. Peter Deutsch's
*RFC 1951: DEFLATE Compressed Data Format Specification version 1.3* (1996),
sections 3.2.5–3.2.6: https://www.rfc-editor.org/rfc/rfc1951.txt. The bounded-window
conceptual credit to Bille, Cording, Fischer and Gørtz remains in the algorithm
notes; this change does not adopt another LZ data structure or parsing algorithm.

The checksum's defining recurrence acknowledges Mark Adler, Jean-Loup Gailly,
and RFC 1950's L. Peter Deutsch/Jean-Loup Gailly (1996):
https://www.rfc-editor.org/rfc/rfc1950.txt. The new reduction is algebra derived
from that recurrence and our own packed sums, not copied implementation code.
Other reviewed/adopted/rejected papers remain distinguished in
[algorithm notes](../algorithm-notes.md), under PR1's mandatory attribution rule.

## Final native results

[Run 38752771416](https://github.com/SirElderling/raipixel/actions/runs/38752771416)
passes all 62 core tests, formatting, Clippy, full-codec exact-byte comparison,
profile/replay checks and scalar-verified bottleneck stage comparisons. Baseline
is 126a556. Complete compressed-encoding time ratios, current/baseline:

| Workload | Linux x86-64 | macOS ARM64 | Windows x86-64 |
|---|---:|---:|---:|
| icon-16 | 0.948 | 0.920 | 0.978 |
| icon-64 | 0.778 | 0.712 | 0.884 |
| noise-256 | 0.878 | 1.148 | 0.907 |
| sheet-1024 | 0.746 | 0.800 | 0.812 |
| texture-1024 | 0.806 | 0.836 | 0.871 |

[Linux raw results](../native-match-adler-final-linux.json),
[macOS](../native-match-adler-final-macos.json),
[Windows](../native-match-adler-final-windows.json) retain every sample, source
fingerprint and the actual-writer profile. All suite outputs match the baseline.
The independent [final reference run](../run-match-adler-final.json) repeats all
fourteen original writer hashes, 31 accepted-profile independent sample checks
for both writers and the multi-block noise checks. The first reference trial is
also retained [here](../run-match-adler.json).

The native isolated checksum comparison measures approximately 32%/24%/24%
less Adler time on the generated sheet for Linux/macOS/Windows respectively.
The matcher screen measures about 24%/34%/24% less time for the sheet, including
fresh encoder/output allocation. These stage measurements do not sum to exact
whole-codec percentages. Benchmark executable growth is 608/16/1,024 bytes on
Linux/macOS/Windows, not a claim about a yet-unimplemented application binary.
No heap size, dictionary size, work limit, filter policy or encoded byte changes.

The macOS whole-codec noise result is **15% slower in this run**. Its isolated
DEFLATE comparison is about 2% slower and its whole-codec decode/stored paths
also drift; that does not explain away or invalidate the observed regression.
The retained version favors the tool's compressible sprite/sheet/texture
workloads with a small source/footprint change; it is not a universal performance
win. Future work should investigate this case rather than treating noise as a
passing timing guarantee. Timing remains informational, not a brittle CI gate.

The original comparison sheet now measures our compressed writer **1.177 ms**
versus reference Fast **0.797 ms**: about **1.48× slower**. Files remain exactly
52,549/53,405 bytes; reference Balanced is 11,233 bytes and takes about 9.30 ms.
Our input decode is 1.438 ms versus reference 0.857 ms. Compare only matching
inputs/protocols: generated native inputs differ from the original reference
corpus, and performance observations are not feature or robustness parity.

[Updated actual writer profile](../writer-profile-match-adler.json) measures the
original sheet's filter/Adler/DEFLATE/IDAT stages at roughly
0.178/0.354/0.681/0.049 ms in the instrumented copy, versus total plain/profiled
writers 1.185/1.286 ms. Keep that overhead visible; the isolated checksum and
whole-codec results are the evidence for an improvement, not a subtraction of
instrumented stage medians. Matching remains the leading target.
