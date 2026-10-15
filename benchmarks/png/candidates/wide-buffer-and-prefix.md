# Complete-codec profiles, batched output and cached prefixes

Baseline: 27243e606e9c5c3ef6fddc0c645255570be75be7, whose production source is
the preceding 8fed9fa implementation. All code is independently authored. The
locked comparison crate is used through public APIs only; its implementation
source is not inspected or copied.

## Build profiles are not a substitute for an algorithm improvement

`tools/png_build_profiles.py` builds the same public-API driver and both libraries
with three explicit, matched release settings: opt-level 3, incremental off,
codegen-units 16/LTO false (default), units 1/LTO false, and units 1/thin LTO.
It rotates the existing seven operations within each process and runs adjacent
default/single/thin/thin/single/default processes **per image**. Fresh allocation,
filtering, work accounting, integrity checks and actual output buffers remain
timed. Builds, image I/O, cross-decoding and output SHA256 checks are untimed.
Every writer's bytes must agree across all profiles/repeats. CPU-native flags
are not enabled. Metadata includes flags, source/input/driver fingerprints,
executable bytes, all raw samples, and the locked reference version.

[Local original-corpus screen](../build-profiles-initial.json) and native run
[38769753624](https://github.com/SirElderling/raipixel/actions/runs/38769753624)
do not justify a production release-profile change. On generated sheets,
single/thin versus default *our* encoder time ratios are approximately
0.928/0.953 Linux, 1.075/1.109 macOS, and 1.090/1.019 Windows. Reference Fast
changes too: Linux's thin-LTO ratio looks closer partly because the reference
takes longer. Do not choose a favorable denominator to claim parity. Smaller
comparison executables alone do not meet the current throughput objective.

Raw baseline profiles: [Linux](../build-profiles-before-linux.json),
[macOS](../build-profiles-before-macos.json),
[Windows](../build-profiles-before-windows.json). The generated native images
differ from the original comparison corpus. The root manifest keeps the default
release settings, stable Rust 1.99 and zero dependencies.

## Retained code changes

### Four-byte bit-buffer flushing

The encoder formerly drained every complete byte after each bit insertion,
performing a byte push/capacity check repeatedly. Its reservoir now uses u64 and
drains four little-endian bytes when the count reaches 32. Pending count stays
below 32 and the largest individual insertion is 13 bits, so at most 44 bits
are live; one four-byte drain suffices. There are no shifts by 64 or speculative
memory accesses. The existing explicit code/extra-bit insertions remain.

At each block boundary all complete bytes are drained, retaining only <8 pending
bits. Alignment adds zeros only to the next byte boundary. The stored fallback
decision therefore sees the same bytes/length, restores the same pending state,
and emits the same data. This also preserves IDAT payload boundaries and CRCs,
not just the concatenated zlib stream. Heap reservations and work charges stay
unchanged; the scalar reservoir grows by four bytes (with platform padding).

### Constant-run scan

For distance-one candidates, compare each 16-byte group with a broadcast of the
preceding byte. General matches still compare two bounded slices. Run selection,
minimum length, maximum length and distance-one tie-breaking are unchanged.
Scanning constant data needs one input group instead of two overlapping loads.
This is the earlier parity screen's candidate, now measured in complete calls.

### Cached prefix in the existing dictionary word

Each block is <=65,535 bytes; only starts <=65,532 are inserted. Therefore a
position fits in 16 bits, and 65,535 is an unused sentinel. Keep the existing
8,192 **u32** entries, storing position in the low half and the first two input
bytes in the high half. The hash function already loads these bytes. Dictionary
allocation/reset traffic stays at 32 KiB rather than adding a second table.

A prefix mismatch proves that the old matcher would reject the candidate. On a
hit, compare the third byte, validate prior position and the 32 KiB window, then
run the existing bounded tail scanner. The block's shared input slice is
immutable, so cached prefixes cannot become stale. This requires no assumption
that the hash is collision-free and no change to hash slots, dictionary updates,
candidate choice, filters, matches or encoded output. The sentinel position is
rejected even when its prefix equals 0xffff.

The first prototype redundantly rechecked all three input bytes after a prefix
hit. It improves noise but slows sheets in the isolated screen. The retained
version avoids rechecking the two already cached bytes. Both keep the third-byte
check; no correctness property is inferred from the multiplicative hash.

## Tests and local screens

Test-first commands for the new run helper and cached matcher first fail with
missing-function errors. Then the implementations pass the scalar/reference
tests. Three new core tests bring the total to **65**:

- Every distance-one termination, including too-short matches and all legal caps.
- A long fixed-symbol sequence checked against an independently assembled
  bit stream at every initial pending-bit offset; it crosses many word flushes.
- Each cached-prefix/third-byte rejection, valid prefix extension and sentinel.

Existing exhaustive overlap/cap tests build dictionary metadata independently
before comparing with scalar match lengths. The fixed-symbol and all
length/distance/pending-bit tests explicitly drain complete bytes before checking
the independent expected byte stream and residual bits. PNG fixture/hash tests
cover complete files, stored fallback and multi-block continuity.

[Four-way buffer/run screen](../emission-screen.json) validates exact complete
DEFLATE bytes, includes fresh allocations, and rotates nine calibrated batches.
The combined version has sheet/noise ratios about 0.788/0.807 but texture 1.058;
these are stage results, not whole-PNG claims. The
[packed-reference follow-up](../emission-reference-screen.json) improves some
stages only a few percent relative to the wider buffer. The earlier rejected
packing experiment used byte-at-a-time output; this retest changes that context.
It still does not justify replacing the clearer explicit insertions. The
[rejected patch](wide-packed-reference.patch) is preserved.

The [first cached-prefix screen](../emission-prefix-screen.json) and
[third-byte-only screen](../emission-prefix-third-screen.json) include duplicate
baseline/candidate modules to expose layout variation. The retained variant's
noise stage takes about 26% less time than buffer/run alone; sheet/texture
results differ by a few percent. These do not replace native complete timings.

A separate [u16-column Adler screen](../adler-u16-screen.json) tests smaller sum
lanes with wider prior accumulators. Width 16 is roughly flat on these local
inputs, while widths 32/64 lose. Equal checksums on the timed inputs are a cost
screen, not full incremental validation. Keep the existing packed checksum;
the [prototype patch](adler-u16.patch) remains unadopted. This introduces no
CPU-specific code, unsafe code or assembly.

[Initial complete buffer/run pair](../timing-wide-run.json) measures current /
baseline compressed time ratios 1.058 icon-16, 0.928 icon-64, 0.755 noise,
0.893 sheet and 1.033 texture. The [cached-prefix pair](../timing-prefix-wide.json)
is 0.960/0.987/0.676/0.841/1.034 respectively. Both verify all generated writer
hashes before timing. These separate runs have different baseline observations;
do not subtract them to claim an incremental prefix-cache speedup.

The [independent buffer/run comparison](../run-wide-run.json) and
[cached-prefix comparison](../run-prefix-wide.json) preserve all fourteen
original stored/compressed hashes, independently verify all 31 accepted PNGs
through both writers, and check multi-block noise. The latest original sheet
takes **1.155 ms versus Fast 0.985 ms**, about **1.17x slower**, at unchanged
52,549/53,405 bytes. The earlier 1.48x observation had Fast at 0.797 ms; the
ratio change alone is not a controlled before/after gain. Decode and stored
encoding are unchanged algorithms and remain timing controls in this pass.

## CI and experiment lifecycle

[Final native code run 38772485077](https://github.com/SirElderling/raipixel/actions/runs/38772485077)
passes 65 core tests, formatting, Clippy, exact writer checks, profiles/replay and
the complete reference/build-profile matrix on all three platforms. Complete
compressed-encoding ratios versus 27243e6 (lower is better):

| Input | Linux x86-64 | macOS ARM64 | Windows x86-64 |
|---|---:|---:|---:|
| icon-16 | 0.954 | 0.935 | 0.981 |
| icon-64 | 0.955 | 0.862 | 0.973 |
| noise-256 | 0.606 | 0.792 | 0.679 |
| sheet-1024 | 0.866 | 0.863 | 0.893 |
| texture-1024 | 0.982 | 0.962 | 0.960 |

Noise takes 21-39% less time, sheets 11-14%, textures 2-4%. The local texture
regression remains in its report; this native run does not turn timing into a
universal guarantee. Baseline/current sheet DEFLATE stages are 0.581/0.455 ms
Linux, 0.525/0.396 ms macOS and 0.555/0.432 ms Windows. The unchanged Adler stage
stays close to baseline; stage times are not additive whole-codec percentages.
Benchmark executable growth is 1,088/144/1,024 bytes respectively; these are
benchmark executables, not the unimplemented CLI's footprint.

[Linux](../native-wide-prefix-linux.json),
[macOS](../native-wide-prefix-macos.json),
[Windows](../native-wide-prefix-windows.json) retain paired samples, fingerprints,
stage comparisons, actual-writer timers and exact token replay. Full reference
profile reports: [Linux](../build-profiles-after-linux.json),
[macOS](../build-profiles-after-macos.json),
[Windows](../build-profiles-after-windows.json).

Current/default build, complete time ratio versus reference Fast:

| Input | Linux | macOS | Windows |
|---|---:|---:|---:|
| icon-64 | 1.160 | 1.177 | 1.109 |
| sheet-1024 | 1.108 | 0.917 | 1.148 |
| texture-1024 | 0.210 | 0.190 | 0.161 |
| noise-256 | 1.284 | 3.064 | 1.847 |

The generated sheet is 0.979/0.883 ms for ours/Fast on Linux,
0.957/1.043 ms macOS and 1.030/0.897 ms Windows. One observed macOS sheet win
does **not** establish general parity. Noise and icon encoding still trail;
noise decode ratios are 4.02/3.24/2.16. Decoder behavior is unchanged in this
pass. Texture Fast files remain much larger, so speed is not interchangeable
with Balanced-size parity. No fixture, mode or platform is removed from the
assessment to hide the remaining gaps.

Native CI keeps the tests, formatting, Clippy, paired complete calls with exact
hash checks, checksum/stage probes, actual-writer profiling, token replay and the
new complete-codec reference/profile matrix. The baseline for current paired
and stage comparisons is 27243e6.

Repeated rejected fusion/XOR/column screens and the deferred decoder-setup
screen are retired from every-commit CI. Their tools and raw records remain.
The historical matcher screens now explicitly default to source revision
27243e6 (`--baseline` overrides it), so later private dictionary changes cannot
silently redefine or break the old experiment. The complete-codec driver uses
public APIs and always tests the current implementation.

The first buffer/run CI found a temporary-source generator deleting the new run
helper. The library tests/hash checks passed; the diagnostic generator was
fixed, locally verified, and all jobs passed in
[38771516778](https://github.com/SirElderling/raipixel/actions/runs/38771516778).
This was a tooling failure, not evidence of a production byte mismatch.

## Attribution

The bit reservoir, fixed codes, three-byte minimum, distance/length limits and
match contract acknowledge L. Peter Deutsch's *RFC 1951: DEFLATE Compressed Data
Format Specification version 1.3* (1996), sections 3.1, 3.2.3, 3.2.5-3.2.6 and 4:
https://www.rfc-editor.org/rfc/rfc1951.txt. Batching, broadcast scanning and cached
prefix packing are independent adaptations of our implementation around that
contract, not copied compressor code. Bille, Cording, Fischer and Gørtz's
bounded-window conceptual credit remains in the algorithm notes; no suffix-tree
algorithm is adopted.

The rejected u16-column prototype independently groups the Adler/Gailly checksum
recurrence, documented by L. Peter Deutsch and Jean-Loup Gailly in *RFC 1950*
(1996), sections 2.2/6: https://www.rfc-editor.org/rfc/rfc1950.txt. It follows the
previous parity review's column algebra and is not a new paper implementation.
Other adopted, rejected and reviewed-only literature remains distinguished in
[algorithm notes](../algorithm-notes.md), following PR1's attribution rule.
