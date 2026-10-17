# Exact palette extraction A/B decision (17 October 2026)

Status: **accept a bounded two-path implementation** for PR #2, subject to
cross-platform post-change CI and regression tests. The old sorting path is
retained only where it is faster or avoids a larger temporary table, and as
the exact-count fallback for overflow or adversarial collisions.

The experiment compares the original scratch-RGB sort (A) with a fixed 2,048
slot / 32-probe counting table (B). Exact results, empty-color behavior,
overflow distinct-count errors and ordinary work-accounting equality passed
in the standalone comparison. Both ignore RGB beneath fully transparent
samples. Timings use seven alternating native release rounds, median nanoseconds
per operation, identical constructed images and deterministic checked output.
This is not measured peak RSS.

## Matched measurements

Absolute times in microseconds. In each cell: **sort / count**.

| Pixels, distinct colors | Linux x86-64 | macOS ARM64 | Windows MSVC |
|---|---:|---:|---:|
| 16×16, 4 | 2.37 / 4.50 | 2.42 / 2.82 | 2.33 / 3.82 |
| 64×64, 16 | 40.43 / 11.32 | 45.20 / 17.41 | 38.24 / 11.01 |
| 1024×1024, 16 | 10620 / 1895 | 10688 / 2802 | 10965 / 1981 |
| 1024×1024, 256 | 17929 / 1939 | 17433 / 3412 | 18084 / 2049 |

Original benchmark run:
[three-platform palette A/B](https://github.com/SirElderling/raipixel/actions/runs/39057777733).

A second independent run adds 32×32 and 48×48 crossover workloads:
[three-platform crossover check](https://github.com/SirElderling/raipixel/actions/runs/39057961888).
Linux fluctuates considerably at small sizes; macOS/Windows favor the counter
at 32×32, while all three favor it at 64×64. To avoid unnecessary scratch for
small AI-authored sprites, use sorting under 4096 pixels.

The sort allocates one RGB key per pixel (3 MiB at 1024²); the counter occupies
49,152 bytes of fixed table scratch plus at most 256 counted-color outputs
and the existing RGBA input. On 16², sorting needs only 768 RGB scratch
bytes; forcing 49 KiB would regress memory and latency. These are logical
scratch sizes, **not sampled RSS or allocator peak measurements**.

## Engineering decision

- Select sorting for <4096 pixels or when the fixed table would exceed the
  configured local byte ceiling.
- Otherwise use the bounded counter with 32-probe limit and 2,048 slots.
  Fallback to the original sort for collision saturation or more than
  max_colors, so exact distinct counts/errors and deterministic ordering hold.
- Preserve original charged work count and transparent-RGB preservation.
  Validate low-byte-limit, clipped-work, empty, overflow and collision cases
  independently; an allocation/work failure must not modify any input.
- No external dependencies, unsafe code, assembly, FFI or reference-png
  implementation knowledge.

The counter is not shown to have lower peak process RSS; stack use and
allocator behavior must remain clearly separated from logical buffer sizes.
Do not generalize the reported speedups to all palettes, host systems or
future workloads. PNG compression output is unaffected because only
palette extraction, not encoding, changes.
