# Row locality and Adler arithmetic trials

## Rejected row compaction fusion

Candidate `6e0babd` moved each existing RGBA row copy immediately after its
unfilter operation, reading the preceding row from its compacted position.
The proof and research acknowledgment remain in the algorithm notes. No bytes,
allocation, resource checks or charged work were removed. All 68 tests, format
and Clippy passed on Linux, macOS and Windows in
[CI 38781977404](https://github.com/SirElderling/raipixel/actions/runs/38781977404).
Exact corpus samples, flags, work remaining and both writer artifacts agreed.

Complete codec ratios versus `a599c19`, same driver/default build profile,
100 ms calibrated batches, seven rotating rounds, adjacent ABBA processes:

| Decode input | Linux | macOS | Windows |
|---|---:|---:|---:|
| Original sheet | 0.970x | 0.985x | 0.992x |
| Original texture | 0.974x | 1.025x | 0.987x |
| Generated icon-16 | 1.032x | 1.060x | 1.033x |
| Generated icon-64 | 0.991x | 0.972x | 1.008x |
| Generated noise-256 | 0.987x | 1.109x | 1.002x |
| Generated sheet-1024 | 0.988x | 1.008x | 0.985x |
| Generated texture-1024 | 0.981x | 1.041x | 1.018x |

Original-sheet stored decode is 1.016x/1.000x/0.999x respectively. Unchanged
writer controls also move: macOS original-sheet stored encode 1.098x,
compressed encode 1.073x; texture compressed encode 1.132x. Its wide sample
ranges indicate runner variability, but do not justify ignoring regressions.
The repeatable 3% tiny-icon regressions on Linux and Windows outweigh the small
large-input benefit for this change. Restore the previous two-loop row layout;
do not add size dispatch to rescue a marginal optimization.

`native-row-fusion-{linux,macos,windows}.json` retain both paired corpora and
actual decoder stages; `build-profiles-row-fusion-*.json` retain all six inputs
against public-API `png =0.18.1` under matched default, one-codegen-unit and
thin-LTO profiles. CI reports a synthetic PR merge revision; library fingerprints
identify candidate sources. Local `timing-row-fusion.json` and
`decode-stages-row-fusion.json` retain the earlier 2-3% decode observations.
These are rejected-candidate results, not current performance claims.

## Adler screening

Both standalone tools use only safe standard Rust and the RFC 1950 recurrence.
They independently check all-255 and patterned data around bulk/reduction
boundaries, every isolated byte coefficient in a 128-byte group, and the full
timed input. Nine rotating calibrated 25 ms minimum batches measure a 4 MiB
buffer; setup and scalar verification are untimed. Source/compiler fingerprints
and all samples are retained. This is checksum throughput, not codec speed.

| Trial | Median microseconds | Relative to current in same run |
|---|---:|---:|
| Dot run: current packed | 380.693 | 1.000x |
| Narrow dot, 16 bytes | 470.161 | 1.235x |
| Narrow dot, 32 bytes | 429.675 | 1.129x |
| Narrow dot, 128 bytes | 445.879 | 1.171x |
| Stream run: current packed | 369.857 | 1.000x |
| Generic one stream | 426.772 | 1.154x |
| Two interleaved streams | 238.253 | 0.644x |
| Four interleaved streams | 359.837 | 0.973x |

Reject narrow dot accumulation: it is 13-24% slower locally. Four streams do not
justify extra arithmetic. Two streams merit a production-shaped trial and
complete-codec measurements on all three native platforms. No checksum change
is adopted on this microbenchmark alone. The generic one-stream spelling is
also slower than the current loop; loop shape matters as well as mathematics.

`tools/png_adler_dot_trial.rs` and `tools/png_adler_streams_trial.rs`, with
`adler-dot-trial.json` and `adler-streams-trial.json`, preserve these experiments.
Compile either from the repository root using `rustc --edition=2024 -O` and run
the resulting executable. Their compact source is retained exactly as measured.

We acknowledge Mark Adler's checksum and L. Peter Deutsch and Jean-loup Gailly's
[RFC 1950](https://www.rfc-editor.org/rfc/rfc1950.html), section 8.2. The packed
and interleaved regroupings are independently derived from its sums; no crate,
dependency implementation, architecture intrinsic or assembly is consulted.
