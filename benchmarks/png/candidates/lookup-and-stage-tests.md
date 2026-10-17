# Lookup, match termination, fusion and Huffman setup experiments

Baseline: 68ee8a64b331b5034239809edb405ee448c3162b. Test/probe checkpoint:
138ad84c2161cb5d2c3c4dd44b9c8c46cbd43e06. All implementation is our own;
no comparison-crate source was read or copied. Native CI also runs the isolated
experiments, not only the retained implementation.

## Stage measurements

`examples/png_stages.rs` includes our actual checksum/DEFLATE modules without
adding production interfaces. Its filter loop reproduces our current writer's
block loop; before timing, it checks every filtered byte against the actual
writer's inflated output. It warms/calibrates to 25 ms, rotates seven stages and
retains all samples. Input must be supported noninterlaced RGBA8: the tool is a
trusted-input experiment, not a general PNG inspector. Allocation is included
where the operation allocates. Parsing/filesystem/setup/reporting is untimed.
The separate stages have different buffer lifetimes/compiler layout from the
whole codec; their times **must not be summed or treated as exact percentages**.

[Initial local stage results](../encoder-stage-baseline.json), original corpus:

| Operation | 1024 sheet | 1024 texture |
|---|---:|---:|
| Sub/Up filtering screen | 0.133 ms | 0.130 ms |
| Adler32 | 0.501 ms | 0.520 ms |
| Fixed-code DEFLATE writer | 0.961 ms | 1.093 ms |
| Output IDAT CRC | 0.028 ms | 0.043 ms |
| Inflate, including Adler | 0.793 ms | 0.859 ms |
| Complete compressed writer | 1.633 ms | 1.840 ms |
| Complete PNG decode | 1.621 ms | 1.085 ms |

These measurements support further encoder/checksum experiments; output CRC is
not the leading candidate on these images. The native suite uses different
original generated pixels; compare matching cases only.

## 1. Encoder lookup tables: compact revision retained

Precompute 286 fixed Huffman code/bit-count pairs and the 256 legal length-code
indices. The tables replace repeated branching/bit reversal and non-258 length
binary searches; they do not alter matching, symbols, filters, work, buffers or
bytes. The initial tuple layout used 1,400 bytes. That version is superseded by the
828-byte packed layout and restored maximum-match shortcut described below.
Before implementation, tests exhaustively checked every emitted fixed symbol
and pending-bit offset, every match-ending byte position/overlapping run, and
the existing reference-bit test checks every length and distance-range endpoint.
There are now **61 core tests**, plus reused private-module tests when running
the stage example with `cargo test --all-targets`.

[Rotating original-input stage screen](../lookup-match-screen.json), nine
batches of 50 newly allocated encoders, shows about 14% less DEFLATE time on
sheet/texture, 8% on noise and 26% on the icon. These are stage results.
The [whole-codec paired run](../timing-lookup.json) with the generated suite
shows sheet encoding 1.460 → 1.357 ms (7.0%), texture 1.823 → 1.648 ms (9.6%),
noise 2.209 → 2.135 ms (3.4%, a small delta), icon-64 8.45 → 7.94 µs, and
icon-16 2.23 → 2.03 µs. All exact suite output hashes match. The same release
benchmark executable grows 607,080 → 608,312 bytes: **1,232 bytes**. This is a
benchmark executable measurement, not a shipping CLI size claim.

## 2. XOR/first-differing-byte match termination: not adopted

The experiment replaces the eight-byte equality/byte-tail fallback with
little-endian word XOR and `trailing_zeros()/8` on a mismatch. It preserves the
first differing byte on every platform; the test-first termination-domain test
covers all possible termination positions through length 258.

The initial screen against the pre-table encoder showed no consistent sheet
improvement and about 8% slower texture encoding. A second reproducible
[screen with lookup tables](../match-trial.json) gives a small sheet delta,
no texture gain and an apparent noise gain. Noise almost never reaches the
word-comparison loop, so that result alone cannot establish that the proposed
mechanism helped. Native repeated screens are needed to distinguish compiler
layout effects/noise from a useful whole-codec gain. Keep current match code.

`tools/png_match_trial.py` compiles current/experimental copies of our own
encoder in one executable, checks complete DEFLATE byte equality first, then
calibrates and alternates nine batches. Python stdlib zlib only extracts trusted
fixture scanlines; it is not part of the application or Rust test dependencies.

## 3. Up-filter/Adler fusion: rejected

`tools/png_filter_adler_trial.rs` checks a fused implementation against bytewise
wrapping subtraction and our actual Adler32. It uses guarded 16-bit subtraction
lanes and our packed checksum recurrence, writing each word while accumulating
its contribution. The 8-bit guard prevents a lane subtraction borrowing into
another lane; the existing checksum sum/weight bounds still apply.

This is deliberately an optimistic screen: contiguous Up input, no row filter
markers, first-row Sub or row-boundary handling. It uses bounded 65,535-byte
scratch and verifies every output byte/checksum before nine alternating batches.
[Local result](../filter-adler-trial.json): separate 0.678 ms, fused 1.285 ms
for 4 MiB. It loses almost 2× even before adding real PNG boundary handling.
No production fusion or additional abstraction is justified by this experiment.

## 4. Dynamic Huffman setup: defer redesign

`tools/png_decoder_profile.py` instruments **a temporary copy of our own**
Huffman constructor with an RAII timer/counter. It retains real parsing,
validation, allocation and work charging. Seven batches of 128 full inflations
record setup time and table count. Instrumentation overhead is included: use
this as a cost screen, not an exact production profile. Sources are fingerprinted;
production decoding is unchanged. The optional `--recompress` flag uses Python
stdlib zlib level 6 on the same scanlines for native generated fixtures.

[Original-corpus profile](../decoder-profile.json): sheet setup 1.95 µs of
802 µs inflate; texture 0.90 µs of 1,044 µs (three tables per decode).
[Recompressed generated fixtures](../decoder-recompressed-profile.json): sheet
4.61/770 µs and texture 1.53/967 µs. Setup is under 1% in all these cases,
including instrumentation. A table-organization rewrite cannot close the
remaining decode gap on these inputs. No new decoder algorithm is adopted.

## Independent reference checks and measurement quality

The original comparison runner now calibrates **each** reference/our operation
to at least 25 ms, warms first, rotates seven batches and retains raw samples and
per-operation iteration counts. It still uses only reference public APIs, fresh
output buffers and integrity checks. The locked crate remains outside root Cargo.
The earlier [short-batch run](../run-lookup-short-batches.json) is retained rather
than silently replaced; use the [calibrated comparison](../run-lookup.json) for
the latest assessment. All 14 writer hashes, both writers on 31 profile fixtures,
and the independent multi-block noise check pass.

The calibrated original sheet comparison is our compressed writer **1.508 ms**,
reference Fast **0.910 ms** (our time is 1.66× reference). Output stays 52,549/53,405 bytes;
reference Balanced stays 11,233 bytes and takes 9.047 ms. Small-case/texture
ratios differ. This does not establish general size, feature or speed parity.

## Attribution

Fixed-code tables, lengths and bit order are implemented from L. Peter Deutsch,
*RFC 1951: DEFLATE Compressed Data Format Specification version 1.3* (1996),
sections 3.2.5–3.2.6: https://www.rfc-editor.org/rfc/rfc1951.txt. The lookup and
XOR variants are independently derived from our existing code and these ranges.
They do not implement another project's encoder.

The experimental fusion concept acknowledges Abhijeet Joglekar, Michael E.
Kounavis and Frank L. Berry, *A Scalable and High Performance Software iSCSI
Implementation*, USENIX FAST 2005, sections 3.2–3.3, freely available at
https://www.usenix.org/legacy/event/fast05/tech/full_papers/joglekar/joglekar_html/.
Our adaptation tests Up filtering with Adler32 instead of their copy/CRC context;
it is rejected, and their published gains are not claimed for this tool.
The checksum definition/recurrence credit remains Mark Adler, Jean-Loup Gailly,
and RFC 1950's authors L. Peter Deutsch/Jean-Loup Gailly (1996), linked in
[algorithm notes](../algorithm-notes.md). PNG filtering is credited to the PNG
Working Group. Nekritch's decoder-table paper remains reviewed, not adopted.
Existing bounded-window conceptual credits also remain in those notes.


## Native results and the revised lookup layout

The [first native trial](https://github.com/SirElderling/raipixel/actions/runs/38737239913)
passes all correctness/output checks, but the initial table layout regresses
macOS sheet encoding by 12%. These results are retained in
[native Linux](../native-lookup-linux.json), [macOS](../native-lookup-macos.json)
and [Windows](../native-lookup-windows.json) reports. It is not accepted as-is.
The initial replacement removed the existing length-258 shortcut and introduced
two serial table lookups for maximum matches, a plausible cost to test.

The follow-up restores the explicit maximum-match path (`symbol(285)`, no length
lookup/extra bits) and stores each code and bit count in a single `u16`: nine low
bits for the reversed code, the remaining bits for its length. This needs one
code-table load instead of separate tuple-field loads and reduces static data to
572+256 = **828 bytes**. This representation is independent of host byte order.
The pre-existing exhaustive symbol/reference-bit tests cover the new layout.

[Alternating layout screen](../lookup-layout-screen.json) and
[paired complete-codec local run](../timing-lookup-packed.json) preserve all exact
bytes. Native [run 38738287332](https://github.com/SirElderling/raipixel/actions/runs/38738287332)
passes 61 core tests, formatting, Clippy, complete-codec comparisons and all four
experiment probes. Its current/baseline compressed-encoding time ratios are:

| Workload | Linux x86-64 | macOS ARM64 | Windows x86-64 |
|---|---:|---:|---:|
| icon-16 | 0.937 | 0.927 | 0.920 |
| icon-64 | 0.913 | 0.855 | 0.924 |
| sheet-1024 | 0.950 | 0.892 | 0.947 |
| texture-1024 | 0.888 | 0.866 | 0.904 |
| noise-256 | 0.907 | 0.823 | 0.882 |

Lower is better. Complete benchmark executables grow **768 bytes on Linux,
144 bytes on macOS, and 512 bytes on Windows**. The native reports preserve both
medians and all samples: [Linux](../native-lookup-packed-linux.json),
[macOS](../native-lookup-packed-macos.json), [Windows](../native-lookup-packed-windows.json).
These are paired shared-runner observations, not platform-wide guarantees.

Fusion loses roughly 1.9–2.2× across the first native runs. XOR termination is
inconsistent and regresses on Windows (first trial: sheet +16%, noise +8%), so
neither experiment is retained. Small Huffman-setup costs remain insufficient
justification for a decoder-table redesign on these workloads.

## Confirming the actual bottleneck

`tools/png_writer_profile.py` builds a temporary instrumented copy of our
library and times the **actual** allocation/preflight, filtering, Adler32,
DEFLATE block call and IDAT CRC/write stages. It checks exact output against the
uninstrumented library, calibrates both versions and alternates nine batches.
Timers are per block, never per token. The uninstrumented/profiled samples expose
instrumentation and compiler-layout effects; the reported percentages are
observations of the instrumented path, not sampling-profiler precision.
Native CI runs this tool too. No profiling API is added to production source.

On the original 1024 sheet, [local actual-writer profiling](../writer-profile.json)
puts DEFLATE at about **51%**, Adler at **33%**, filtering **10%**, and IDAT
CRC/writing **3%** (medians of per-round shares). Texture has the same priority;
noise spends about **89%** in DEFLATE. Actual-writer native profiles are also in
the three packed reports above. Profiling perturbs timings; the median full
writer costs are recorded alongside each stage sample rather than hidden.

`tools/png_token_replay.py` captures the actual literal/reference sequence
outside timing and replays our real bit emitter, headers, alignment and stored
fallback. It verifies complete DEFLATE byte equality against the actual matcher
before measuring. [Local generated sheet](../token-replay.json): 582 literals,
19,890 references; full DEFLATE 0.661 ms, emission replay 0.154 ms. Texture is
1.015/0.333 ms, noise 1.910/0.774 ms. Different token-memory/cache behavior means
**subtracting these is not an exact match-time measurement**. It nevertheless
supports investigating hash/match work before another bit-packing rewrite.

The next focused work should target matching/hash traversal and Adler arithmetic.
Filtering/checksum fusion failed its cost screen, and dynamic Huffman setup is a
small fraction of decode on these fixtures. No general parity claim follows.

## Final independent comparison after the compact revision

[The final calibrated reference run](../run-lookup-packed.json) repeats all
profile/noise/sample and 14 exact-hash checks for the retained code. The original
1024 sheet takes **1.483 ms** for our compressed writer versus **0.813 ms** for
reference Fast: **1.82× slower**, with the same 52,549/53,405 bytes. The preceding
tuple trial's 1.66× ratio used a slower reference observation (0.910 ms); these
separate runs must not be treated as a controlled before/after speedup. The
native same-host paired comparisons are the evidence for the new change.

## Timing drift follow-up

A repeated macOS suite-order run at 850798e4 illustrates why the control
operations are retained: texture compressed encoding rises 13%, but unchanged
stored encoding also rises 13%, and unchanged decoders rise 22–28%. The earlier
all-platform improvements are observations of their run, not a stable macOS
performance guarantee. Raw repeat samples remain in
[run 38739180001](https://github.com/SirElderling/raipixel/actions/runs/38739180001).
The runner is therefore tightened to ABBA **per case**, generating/timing just
one image for adjacent baseline/current processes. This changes only benchmark
scheduling, not library behavior. A protocol marker distinguishes newer records
from previous whole-suite process ordering. The bottleneck evidence is much
stronger than the smaller macOS before/after deltas.
