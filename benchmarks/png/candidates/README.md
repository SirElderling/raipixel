# Next encoder candidate: run-first matching and direct code mapping

Status: isolated prototype, **not adopted in production source**. Baseline is
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

## Next testing gate

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
