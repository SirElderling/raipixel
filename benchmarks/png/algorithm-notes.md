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
