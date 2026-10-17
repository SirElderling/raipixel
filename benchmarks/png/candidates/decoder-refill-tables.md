# Decoder bit-refill and Huffman-table experiments — 15 October 2026

**Decision: reject all three decoder changes; restore the last measured production codec.**
This record covers independent changes to raipixel's own RFC 1951 decoder.
The comparison dependency `png =0.18.1` was used **only through public APIs**,
in the already-existing isolated comparison harness. Neither its source nor its
implementation was inspected.

## Hypotheses and validation

1. **Exact-size bulk refill**: replace repeated per-byte `Work::charge(1)`
   calls with a single charge for precisely the bytes required by a Huffman
   lookup or a stored-block read. Test truncated input, pending bits and valid
   work accounting. The new precharge changes partial consumption when a
   multi-byte refill fails for insufficient work; this is another reason not
   to keep it without compelling speed evidence.
2. **Packed Huffman entries**: represent each table entry with a single `u16`
   holding the symbol (at most 285) and code width (at most 15), rather than
   a padded four-byte structure. Check all representable symbol/width pairs.
   Tables use half the memory, but fewer bytes alone do not imply throughput.
   Tested both with bulk refill and in isolation.
3. **Unrolled refill**: expand the common first two exact single-byte reads
   directly and keep the previous loop as a fallback for any larger future
   request. This preserves the exact per-byte charge and truncated-input
   behavior. Added a scalar oracle over partial buffers, widths and offsets.

No external algorithm/code was imported. The format and code lengths are
grounded in RFC 1951 (L. Peter Deutsch, 1996); resource accounting is our own.
The existing RFC 1950 and PNG creator acknowledgments remain unchanged.

Each version passed native Rust tests, formatting, Clippy, complete PNG byte
hashes, the independently checked original RGBA digests, decoder metadata and
remaining-work checks for successful decodes on Linux, macOS ARM64 and Windows.
All generated writer files stayed identical. CI uses the existing
same-driver, calibrated, adjacent ABBA process protocol with the fixed
baseline `9122259844497a57149dbe6ba7fa0ef708398c46`.

## Original 1024×1024 comparison-sheet decode

Observed **current / fixed-baseline** complete-decode time (lower is faster).
The accepted row is from the preceding two-stream-Adler run; each other row
comes from its own native run, paired against that same fixed baseline.
Differences *between* rows are not themselves a directly controlled A/B test.

| Decoder source / PR run | Linux | macOS | Windows |
|---|---:|---:|---:|
| Accepted source, [run 38796462226](https://github.com/SirElderling/raipixel/actions/runs/38796462226) | 0.882 | 0.929 | 0.932 |
| Bulk refill, [run 38810396087](https://github.com/SirElderling/raipixel/actions/runs/38810396087) | 0.996 | 0.977 | 0.979 |
| Bulk refill + packed entries, [run 38810626333](https://github.com/SirElderling/raipixel/actions/runs/38810626333) | 0.992 | 1.085 | 1.039 |
| Packed entries only, [run 38811189984](https://github.com/SirElderling/raipixel/actions/runs/38811189984) | 0.901 | 0.920 | 0.935 |
| Unrolled refill only, [run 38812192534](https://github.com/SirElderling/raipixel/actions/runs/38812192534) | 0.891 | 0.901 | 0.981 |

The bulk-refill version erases much of the earlier sheet improvement,
and its icon-64 decode is also markedly worse than the accepted run.
Adding a compact table does not rescue that result.
Packed-only is around the original-sheet baseline on Windows, better on
macOS, and slower on Linux; other workloads are mixed.
The unrolled version's modest sheet improvement on macOS does not reproduce
on Windows, where original-sheet and original-texture ratios rise to
0.981 and 1.037 (accepted 0.932 and 0.930).
For generated icon-64 the unrolled version is 0.893 / 0.997 / 0.962,
versus accepted 0.837 / 0.901 / 0.880 on Linux / macOS / Windows.
These outcomes do not justify extra hot-path branching.

The linked workflow logs preserve compiler flags, source/input hashes,
full timing samples, output hashes, reference-mode measurements, and all
three matched release build profiles. Their reference timing ratios also
vary substantially between otherwise similar native runs; never ascribe
small reference differences to these changes. No timing threshold was
used as a CI pass/fail gate.

## Retained state and remaining gap

The decoder source was restored **byte-for-byte** to accepted blob
`addf43d8212c8a93f62d74a22d83a36259312137` after the last trial.
No new dependencies, unsafe code, CPU dispatch, codec behavior or feature
claims were introduced.

The last accepted matched-default public-API comparison, recorded in
[`build-profiles-adler-streams-*.json`](../), still reports original-sheet
decode at roughly **1.19× / 1.15× / 1.02×** `png 0.18.1` time on
Linux / macOS / Windows respectively; generated noise decode remains around
**2.47× / 2.07× / 2.33×**. Some sheet compressed-encode timings are already
close to or faster than the reference Fast mode, but there is no general
feature or performance parity. The codec remains static-only and rejects
16-bit inputs and APNG.

**Next evidence-based target:** profile the literal-heavy inflate path and
the existing work-accounting overhead on the noise case using a controlled
same-source A/B experiment. Do not repeat these rejected table/refill
spellings or promote a single favorable shared-runner sample.
