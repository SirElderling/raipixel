# Open algorithm literature and PNG decisions

Reviewed 2026-10-15. These are freely readable primary papers, not dependencies
or implementation sources. Recommendations below are our engineering judgement
for raipixel's safe Rust, bounded-memory and KISS requirements. Published speedups
are not predictions for our hardware, images or compiler.

| Paper / accessible full text | Relevant result | Decision for this implementation |
|---|---|---|
| Joglekar, Kounavis, Berry, [A Scalable and High Performance Software iSCSI Implementation](https://www.usenix.org/legacy/event/fast05/tech/full_papers/joglekar/joglekar_html/), USENIX FAST 2005, sections 3.2–3.3 | Slicing-by-eight CRC uses eight small tables; interleaving copying and checksumming can improve locality and instruction overlap. | Our independently derived CRC already uses slicing-by-eight. Copy/checksum fusion is a possible measured experiment, not an automatic rewrite. The paper uses CRC32c: PNG requires its own CRC32 polynomial, initialization and finalization. |
| Yakov Nekritch, [Decoding of Canonical Huffman Codes with Look-Up Tables](https://theory.cs.uni-bonn.de/ftp/reports/cs-reports/1999/85209-CS.pdf), Bonn report 85209, 1999, sections 1–2 | Lookup decoding trades table size and initialization work against decoding steps; a complete maximum-width table can cost too much for short inputs. | Our full tables and compile-time fixed tables already exploit lookup decoding. Consider a small first-level table with a long-code fallback only if profiling shows dynamic table construction/cache cost matters. Preserve DEFLATE's reversed bit packing and malformed/degenerate-tree checks. |
| Bille, Cording, Fischer, Gørtz, [Lempel-Ziv Compression in a Sliding Window](https://drops.dagstuhl.de/storage/00lipics/lipics-vol078-cpm2017/LIPIcs.CPM.2017.15/LIPIcs.CPM.2017.15.pdf), CPM 2017, DOI 10.4230/LIPIcs.CPM.2017.15, sections 1–2 | Exact sliding-window parsing with space proportional to the window; the discussion explicitly distinguishes practical hash-based matchers from more elaborate suffix-tree indexing. | Start with a bounded hash matcher, not the paper's suffix-tree algorithm. Restrict references to DEFLATE's 32 KiB distance and 258-byte maximum length. An approximate match search may miss compression opportunities but must never change decoded bytes. |

## Useful literature to defer

[Kärkkäinen, Kempa, Puglisi, Linear Time Lempel-Ziv Factorization: Simple,
Fast, Small](https://arxiv.org/abs/1212.2952), 2012 preprint, studies efficient
whole-string factorization. Its auxiliary structures and objective differ from
our bounded DEFLATE writer. It is useful background, not the first encoder to
build. We reviewed its abstract/space bounds, not an implementation.

[Drijvers et al., Fast Huffman Decoding by Exploiting Data Level
Parallelism](https://alexandria.tue.nl/openaccess/Metis244492.pdf) studies vector/
VLIW processing. Its hardware assumptions do not establish gains for portable
safe Rust. Defer architecture-specific work; do not weaken the project's safety
constraints to reproduce a paper's benchmark.

## Standards are the immediate algorithm contract

The [PNG Third Edition, section 12.7](https://www.w3.org/TR/png-3/#12Filter-selection)
provides a directly relevant adaptive filtering heuristic: compare signed-byte
residual costs. This is an encoder heuristic, not a guarantee of smallest
DEFLATE output. Start with inexpensive Sub/Up filtering and measure whether more
filter choices justify their cost.

[RFC 1951, sections 3.2.5–3.2.6 and 4](https://www.rfc-editor.org/rfc/rfc1951.txt)
defines the exact length/distance ranges, fixed canonical Huffman codes, bit
packing and practical hash-based matching discussion. The first compressed
writer should use fixed codes, bounded match search and stored-block fallback.
Dynamic Huffman generation, exhaustive parsing and suffix indices should wait
for evidence that the simple writer is insufficient.

Compression/filter choices change PNG file bytes while preserving every RGBA
sample, including transparent RGB. Keep the existing stored writer's byte
contract. Verify a new optional writer with independent decoding, exact sample
checks, deterministic serialization and the same native three-platform CI.
No source from the comparison crate has been inspected or copied.

## Attribution for the first compressed writer

`png::encode_compressed` and private `deflate::encode::Encoder` implement our
own bounded, block-local hash matcher with greedy matches, fixed Huffman codes
and stored fallback. The wire format, fixed codes, match limits and hashing
background come from P. Deutsch's RFC 1951; PNG Sub/Up transforms come from the
PNG specification by the PNG Working Group.

The bounded-window design rationale and explicit decision to favor a practical
hash matcher over exact suffix-tree parsing acknowledge Philip Bille, Patrick
Hagge Cording, Johannes Fischer and Inge Li Gørtz, *Lempel-Ziv Compression in a
Sliding Window*, CPM 2017, linked above. This is a partial conceptual influence,
not an implementation of their algorithm or a claim to its exact-parse bounds.
Our simplifications are one recent candidate per hash plus a distance-one run candidate, a dictionary reset per
65,535-byte block, a 32 KiB maximum distance and 258-byte maximum match.

The slicing-by-eight CRC technique is credited to Abhijeet Joglekar, Michael E.
Kounavis and Frank L. Berry's FAST 2005 paper above. Our polynomial-based CRC
code predates this literature review and was independently authored; the
reference is added to acknowledge the published technique. Copy/CRC fusion has
not been adopted. Nekritch's alternative lookup organization, the exact suffix
structures and the vector/VLIW techniques remain reviewed, not implemented.

Original implementation and tests remain subject to the mandatory
[research attribution requirements](../../docs/design/01-research.md#research-attribution-requirements).

## Adler32: grouping the defining recurrence

The checksum contract is credited to Mark Adler and the zlib format work of
Jean-Loup Gailly and Mark Adler, documented by L. Peter Deutsch and Jean-Loup
Gailly in [RFC 1950, ZLIB Compressed Data Format Specification version 3.3,
1996, sections 2.2 and 6](https://www.rfc-editor.org/rfc/rfc1950.txt).
`checksum::Adler::update` independently regroups the specification's two sums;
no third-party checksum implementation is used.

For 128 bytes, let `S[j] = sum(x[8*k+j])` for `k=0..15`, and
`P[j] = sum((15-k)*x[8*k+j])`. The defining recurrence gives:

```text
A' = A + sum(S[j])
B' = B + 128*A + sum(8*P[j] + (8-j)*S[j])
```

This follows directly from each byte's weight:
`128-(8*k+j) = 8*(15-k)+(8-j)`. The implementation packs the four even and
four odd byte positions into two u64 words with 16-bit lanes. Each sum lane is
at most `16*255 = 4080`; each prior-sum lane is at most
`(0+...+15)*255 = 30600`. Therefore ordinary word additions cannot carry
between lanes. Explicit little-endian loads preserve byte positions on every
host. There are no architecture intrinsics, unsafe blocks or heap allocations.

Full A/B values remain in u64 and are reduced modulo 65,521 after at most
65,535 bytes; even all-255 input stays below 2^40 in B. The remaining bytes use
the defining scalar recurrence. Tests isolate every one of the 128 byte
weights, compare with scalar calculations, use full all-255 blocks and exercise
fragmented updates across the batch boundary. These tests were pushed before
the production regrouping.

This optimization changes neither PNG serialization nor the work-limit policy.
Both stored and compressed benchmark output hashes are checked against their
respective earlier records. The earlier weighted eight-byte loop and an
exploratory multiplication-based packed sum were measured before selecting the
128-byte grouping; the latter was simpler but materially slower in the probe.
The existing reviewed-only Huffman/vector papers remain reviewed-only; this
checksum regrouping is derived from the checksum recurrence above.

## Distance-one overlap

`deflate::inflate` uses a direct byte fill for a distance-one history reference.
This is the overlap behavior specified by L. Peter Deutsch in RFC 1951 section
3.2.3, linked above: every referenced byte becomes the preceding byte. It does
not adopt another paper's parsing/matching algorithm. The fill follows the same
history/window, exact output-length, allocation-capacity and work-budget checks
as the general copy path. The length-258 fixture spells out the fixed codes
independently and is checked by standard zlib before the Rust optimization.


## Byte-preserving encoder search/code adjustments

Run-first maximal matching, direct distance-code arithmetic and the length-258
shortcut in `src/deflate/encode.rs` were independently derived from our original
matcher and L. Peter Deutsch's RFC 1951 (1996), sections 3.2.5–3.2.6, linked
above. The no-run branch keeps literal-heavy inputs out of run selection.
These are adaptations of our implementation around the standard's defining
ranges, not a new paper algorithm or code from the comparison crate. The
existing bounded-window conceptual credit to Bille, Cording, Fischer and Gørtz
continues to apply. A code/extra-bit packing experiment was measured and rejected;
no additional literature algorithm was adopted. See the candidate and timing
records for our measurements, independently of any published performance claims.


## Lookup tables and measured alternatives

The [lookup/stage experiment record](candidates/lookup-and-stage-tests.md)
documents the encoder's compile-time fixed-code and length maps derived from
Deutsch's RFC 1951, the independent XOR termination experiment, and exact tests.
No comparison-crate source was used. Our dynamic decoder table construction is
profiled, not redesigned; Nekritch's alternative remains reviewed-only.

The filter/Adler prototype in `tools/png_filter_adler_trial.rs` partially uses
the fusion concept discussed by Joglekar, Kounavis and Berry (FAST 2005, linked
above). It adapts copy/CRC fusion to Up-filter generation and Adler32; this
prototype is rejected after measurements, not retained in production. Its packed
checksum recurrence still acknowledges Adler, Gailly and RFC 1950 as above.
The guarded subtraction and checksum code were independently derived. Paper
results are distinct from our own paired timings. The record links raw samples,
source locations and adaptations as required by PR1's attribution guideline.

The retained lookup version packs each reversed fixed code and bit count into
one `u16` and preserves the original maximum-length shortcut. It is derived from
our code and the RFC 1951 ranges, with 828 bytes of static data. Actual-stage
instrumentation and token replay only analyze our own implementation; they add
no algorithm attribution claim or production API. Full adaptations and evidence
are in the lookup/stage decision record linked above.

## Bounded match views and direct packed-lane reduction

The match optimization in `src/deflate/encode.rs` keeps candidate rejection in a
small inline guard, then scans two already bounded byte slices in 16-byte groups
and finishes with one scalar remainder loop. Keeping the larger scan out of the
guard avoids expanding the common failed-candidate path. Match selection,
window/length limits and tie-breaking still follow our original matcher and
Deutsch's RFC 1951. No third-party match-search implementation is used.

For the independently regrouped Adler recurrence, let `E_i`, `O_i` denote the
four 16-bit even/odd sum lanes and `PE_i`, `PO_i` their prior-word sum lanes.
With `S_i = E_i + O_i`, `P_i = PE_i + PO_i`, for `i=0..3`:

```text
A' = A + sum(S_i)
B' = B + 128*A + 8*sum(P_i) + 2*sum((4-i)*S_i) - sum(O_i)
```

This is the existing byte-weight expression with its even/odd terms collected;
the recurrence credit remains Mark Adler, Jean-Loup Gailly and RFC 1950's
L. Peter Deutsch/Jean-Loup Gailly (1996), linked above. The code in
`checksum::Adler::update` computes these totals directly rather than extracting
and updating A/B once per lane. This algebra was derived from our implementation
and the defining recurrence; no new paper algorithm or comparison code is used.

`E_i,O_i <= 4080`, so `S_i <= 8160` and `sum(S_i) <= 32640`. Consequently a
wrapping multiply by `0x0001000100010001`, followed by a 48-bit shift, gives the
four-lane sum: no lower 16-bit prefix sum can carry into the next field; terms
above bit 63 are intentionally discarded. The odd lanes obey a tighter bound.
`P_i <= 61200`; their total may exceed 16 bits, so they are first collected in
two 32-bit fields (each <=122400) and then summed. The weighted S total is at
most 81600. The unchanged 65,535-byte reduction interval and u64 A/B bounds still
apply. Exhaustive single-byte weights, all-255 full blocks, scalar checksum and
fragmentation checks validate these identities independently.

Two/four interleaved prefix streams were also screened. Once direct reduction
was added, the serial prefix version was within a few percent of the two-stream
version locally, so the retained code keeps the simpler original prefix loop.
Raw performance records distinguish prototypes from retained implementation.

## Parity review prototypes

The new safe column-Adler, u16 dictionary and constant-run diagnostic prototypes
are described and attributed in the [parity review](candidates/parity-review.md).
Their algebra follows our existing independent RFC 1950 grouping; they preserve
the RFC 1951 match limits. No new paper algorithm, external implementation,
production SIMD/unsafe/assembly code or codec feature is adopted. The record
distinguishes generic native CI from CPU-specific code-generation diagnostics.

## Batched output and dictionary prefix caching

The [complete-codec decision record](candidates/wide-buffer-and-prefix.md)
documents and attributes four-byte bit-buffer flushing, constant-run broadcast
scanning and cached two-byte prefixes in the existing u32 dictionary slots.
These are independently derived changes around L. Peter Deutsch's RFC 1951
(1996), with unchanged token selection and file bytes. Block-local positions
fit in 16 bits; the other 16 bits cache immutable input bytes. The remaining
third byte and all history/window limits are checked normally, without assuming
collision-free hashing. Pending bit counts and block-boundary drains are proved
in the record and checked against independent bit streams/output hashes.

The rejected smaller-column Adler prototype retains the Adler/Gailly and
RFC 1950 recurrence credit above. No new paper implementation or CPU-specific,
unsafe or assembly code is adopted. Historical rejected screens remain
reproducible at an explicit source revision; native CI now measures the selected
codec and the locked reference through complete public operations.


## Wider CRC slicing for stored PNG payloads

The implementation is in `src/checksum.rs` (`crc_tables` and `crc32`).
The actual decoder-stage profile identifies CRC32 validation as the main cost
on the generated noise input (about 145 of 200 microseconds locally). We extend
our polynomial-generated slicing tables from eight to 32 byte positions, with
an eight-byte remainder loop and scalar tail. The published slicing technique
is credited to Abhijeet Joglekar, Michael E. Kounavis and Frank L. Berry,
*A Scalable and High Performance Software iSCSI Implementation*, USENIX FAST
2005, sections 3.2–3.3, linked above. The 32-byte grouping is our independent
extension; no source from the paper, `png`, or its dependencies is copied.

Let `T[k][v]` be the CRC contribution of byte `v` followed by `k` zero bytes,
using PNG's reflected polynomial `0xedb88320`. For a 32-byte group, XOR the
incoming remainder into its first four bytes in little-endian order. The new
remainder is the XOR of `T[31-i][byte[i]]` for `i=0..31`. These are the same
polynomial contributions as four successive eight-byte groups; initialization,
final complement, byte order and fragmented-part handling stay unchanged.
The static table grows from 8 to 32 KiB, with no heap or work-budget change.
The paper's CRC32c polynomial is not used for PNG.

Before modifying production code, we added a regression against the independent
bitwise polynomial recurrence, covering every length through 129, larger block
boundaries, empty parts and splits through 65. It passes on both the existing
correct implementation and the candidate. `tools/png_crc_trial.py` freezes the
original source at `09c7359` and screens 8/16/32-byte versions with nine rotating
calibrated batches. `tools/png_decode_stages.py` instruments only a temporary
copy, checks its pixels against the plain build, and records plain/instrumented
timings. Its nested CRC and Adler timers must not be added to their parent
stages. Native CI measures the complete codec as well as these probes.

The initial native 25 ms pass improves noise/stored throughput but shows a
macOS compressed-sheet regression amid large timing stalls. The follow-up
raises the complete-codec minimum batch to 100 ms; both runs are retained in
the decision record, and the initial regression is not discarded.

The 100 ms repeat still gives mixed compact-input timings, including macOS
icon and Windows sheet regressions. The next candidate retains the original
eight-byte bulk loop for parts smaller than 4 KiB, using wider slicing for
larger parts. This is a measured payload-size policy, not a format restriction:
large stored/noise payloads benefit consistently, while small compressed IDAT
parts do not justify the wider table working set. Tests cover both sides of the
4 KiB crossover. Both unrestricted-width runs remain recorded.
