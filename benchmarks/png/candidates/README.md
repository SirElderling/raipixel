# Next encoder candidate: run-first matching and direct code mapping

Status: run-first/direct-code implementation adopted after adding the paired
full-codec harness and exhaustive Rust distance-code test. Baseline is
448be5f00eacbcaae72c912e1866d8671f080b47. Investigated 2026-10-15.
The patches target only private `deflate::encode::Encoder`; no API, allocation,
filter policy, dictionary size, work budget or output format changes.

## Priority and mechanism

1. **Run-first matching with early exit.** Our encoder currently scans the hash
   candidate, then scans a distance-one run, often examining the same constant
   bytes twice. Check the run first. If it reaches the maximum available match,
   skip the other scan: it cannot be longer, and the existing tie-break already
   chooses distance one. Keep the hash-table update at the original position so
   later greedy choices stay identical. Otherwise compare both as before.
2. **Direct distance-code mapping, plus the length-258 shortcut.** Replace the
   binary range search for distance codes with the arithmetic pattern in the
   defining table. Let `x = distance-1`. For `x<4`, the code is `x`; otherwise
   `n=floor(log2(x))` and the code is `2*n + ((x >> (n-1)) & 1)`. Keep the
   existing base/extra tables for emission. Length 258 directly selects index
   28; other lengths retain the existing search.

The first changes search order while preserving the chosen match. The second
changes how we compute the same code index. These are small adjustments to our
current greedy matcher/fixed-code encoder, without a new compression framework.
The distance formula was checked against the table for all 32,768 valid values.

The wire-format ranges and fixed codes are credited to L. Peter Deutsch,
[RFC 1951, DEFLATE Compressed Data Format Specification version 1.3, 1996,
sections 3.2.5–3.2.6](https://www.rfc-editor.org/rfc/rfc1951.txt).
These adjustments were independently derived from that table and our own
implementation. Existing conceptual research influences and creators remain
acknowledged in [the algorithm notes](../algorithm-notes.md). No source from the
comparison crate was inspected or copied.

## Screening and complete PNG trials

[Stage screening](../candidate-stage-screen.json) compares the two changes
separately and together, in one executable with rotating execution order:

| 1024 sheet, DEFLATE stage only | Median | Reduction from baseline |
|---|---:|---:|
| Current matcher/codes | 1.625 ms | — |
| Run first | 1.156 ms | 29% |
| Code mapping | 1.286 ms | 21% |
| Combined | 0.952 ms | 41% |

This stage includes fresh encoder/output allocation, but excludes filtering,
checksums and PNG framing. It is not the complete writer time.

The [baseline](../candidate-baseline.json) and [combined trial](../candidate-combined.json)
then measure the full writer. The initial sheet result is 2.33 → 1.58 ms, but
reference timings were also faster in the later process. We therefore repeated
three inputs in baseline/combined/combined/baseline order, each process using
seven rotating benchmark batches: [raw repetitions](../candidate-repeats.json).

| Complete compressed writer | Baseline repeats | Combined repeats |
|---|---:|---:|
| 64×64 icon | 10.05–10.94 µs | 9.55–9.98 µs |
| 1024×1024 sheet | 2.24–2.34 ms | 1.66–1.67 ms |
| 1024×1024 texture | 2.08–2.19 ms | 1.94–1.97 ms |

The repeated sheet gain is about 25–29% lower total encoding time. Small-icon
results are close enough that we do not use them to establish a substantial
improvement. The reference Fast sheet encoder in the combined repeats takes
0.79–0.81 ms, so this candidate still takes about twice as long. Sheet output
remains exactly 52,549 bytes. No size/parity superiority is claimed.

In the isolated checkout, all 57 local tests, formatting and Clippy pass. The
complete comparison preserves all fourteen stored/compressed PNG SHA256 values,
independently checks both writers on the 31 profile fixtures and checks the
multi-block noise output. This is stronger than only a self-round trip.

## Historical testing gate (now exercised by the paired harness)

Recommend the **combined** patch as the next production trial. Before adoption:

- Add a Rust test comparing direct distance mapping with the defining table
  over the complete valid distance domain; retain the existing writer-byte,
  resource, overlap and independent decoding checks.
- Run native Linux/macOS/Windows correctness and full-writer before/after
  measurements. The existing checksum probe does not measure this encoder change.
- Confirm exact writer bytes, unchanged bounded scratch/work behavior and no
  meaningful regression on the existing icon, sheet, texture and noise inputs.
  Keep timing informational rather than using a brittle CI threshold.

Apply either individual patch or the combined patch, not all three in sequence.
From a checkout named `raipixel`, a repeatable whole-codec trial is:

```sh
git worktree add --detach ../raipixel-encoder-trial 448be5f
cd ../raipixel-encoder-trial
git apply ../raipixel/benchmarks/png/candidates/combined.patch
cargo fmt --all -- --check
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
python tools/compare_png.py --output /tmp/candidate-trial.json
```

The optional comparison runner needs Pillow and the separately locked reference
crate. Production and Rust tests remain dependency-free and offline.

Dynamic Huffman emission is a separate future **size** experiment if we choose
to pursue the Balanced-size gap. It adds code-length generation and header
encoding and changes PNG bytes. It is not the first speed experiment while these
small, byte-preserving adjustments show a measured gain. Larger suffix indices,
match-search chains and decoder-table redesign are not justified by this trial.


## Full-codec integration

The permanent [timing protocol](../timing.md) now compares complete codec calls
on all three native CI platforms, including fresh allocations and exact writer
SHA256 equality. Local [control](../timing-control.json) runs show several-percent
temporal drift (one unchanged texture decode differs by 13%). The first paired
[implementation run](../timing-run-first.json) reduces the generated sheet's
compressed encoding median from 1.942 to 1.427 ms (26.5%) and texture from 2.098
to 1.945 ms (7.3%). Noise encoding rises 2.301 to 2.479 ms (7.7%); this is an
observed potential regression requiring subsequent measurement, not a claimed
improvement. These generated icons/sheet differ from the original comparison
corpus, so compare only matching cases within each report.

The next small experiment packs the length code with its extra bits and the
distance code with its extra bits before calling the bit writer. It aims to
reduce four bit-buffer updates to two without changing tokens, bytes, allocation
or format. A test first compares emitted bytes **and pending bits** against the
original four-call sequence for every length, both endpoints of every distance
range and all eight pending-bit offsets. Native results and independent crate
comparison remain required before reporting a cross-platform improvement.


## Follow-up decision

The bit-packing trial is **rejected**. Its [paired report](../timing-packing-rejected.json)
shows sheet writing 1.573 → 1.388 ms, but unchanged input decode also falls
1.152 → 1.052 ms. Nine alternating isolated DEFLATE batches (50 fresh encoders
per batch) give sheet medians 0.921/0.934 ms and texture 1.117/1.107 ms for
original/packed updates. That does not establish an improvement; retain the
simpler existing bit writer. The permanent reference-bit test remains useful.

The initial native run-first trial confirmed sheet improvements on all three
systems but exposed noise regression on Linux (8.2%) and Windows (4.1%). The
follow-up explicitly handles `repeated == 0` with the hash candidate, avoiding
run-selection comparisons for that common literal-heavy case. For positive
runs, retain the maximal-run early exit and original distance-one tie-break.
This also makes the validity of `pos-1` explicit: a positive run requires a
preceding byte. No dictionary update, match choice or encoded byte changes.
An alternating stage probe reduced noise 2.331 → 2.143 ms while sheet/texture
remained within about 1%; the final whole-codec record is
[here](../timing-matcher.json). This branch adjustment is adopted and measured
again in native CI. Source/compiler/workload details accompany the reports.
