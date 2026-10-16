# Safe DEFLATE distance-2/4 overlap copy experiment — 16 October 2026

## Decision: reject the special-case copying loops

Keep the existing safe `extend_from_within`/bounded overlapping-copy implementation in `src/deflate.rs`. A temporary-copy trial of repeated 2-byte and 4-byte pattern fills does **not** improve general PNG decoding and **severely regresses cases that directly exercise those patterns**. The production decoder remains unchanged.

No third-party `png` crate implementation was inspected. No new dependency, `unsafe`, inline assembly, intrinsics, or platform-specific code was added. All native measurements use Rust **1.99.1**.

## Evidence

[Native Rust workflow](https://github.com/SirElderling/raipixel/actions/runs/38863254035) runs the reproducible [scratch benchmark](../../../tools/png_overlap_trial.py) comparing the accepted decoder, a distance-2-only specialization, a distance-4-only specialization, and both. It checks complete decoded PNG pixels, flags, remaining work units and the rejection of corrupted PNG chunk CRCs. Nine rotated batches are calibrated to at least 25 ms per operation and include allocation and full PNG parsing.

Two fixed-Huffman PNGs are **independently constructed in Python from the RFC 1951 bit format**, verified with Python's standard-library zlib decompressor, and contain explicit 258-byte matches at distance 2 and 4. They ensure the trial actually exercises the new branches. This is stronger than inferring overlap behavior from ordinary images.

Ratios are candidate complete-PNG decode time divided by the accepted implementation; **1.0 is parity and below 1.0 would be faster**.

| Workload | Linux distance 2 | Linux distance 4 | Windows distance 2 | Windows distance 4 |
|---|---:|---:|---:|---:|
| Crafted distance-2 PNG | **1.401** | 1.019 | **1.475** | 1.011 |
| Crafted distance-4 PNG | 1.004 | **1.552** | 1.019 | **1.640** |
| Generated sheet 1024 | 1.004 | 1.001 | 1.002 | 1.005 |
| Generated texture 1024 | 1.005 | 1.008 | 1.001 | 1.005 |
| Original mixed-filter sheet 1024 | 1.012 | 1.011 | 1.003 | 1.000 |

The crafted fixture results show **40–64% longer runtime when the matching specialization is invoked**, without compensating whole-PNG benefits elsewhere. Results are from the `69f9a86b3` run; macOS ARM64 was queued at the time of this report, and no ARM64 claim is made.

## Why not retain conditional code?

The dedicated fill loop first resizes the output, then stores one output byte at a time based on a small pattern. It preserves samples but adds branching and length-dependent per-byte writing. The original bounded copy can exploit optimized slice copying; there is no measured case for complicating a hot decoder loop with the new branches.

This specifically rejects *these two fill loops*, not every possible safe Rust improvement to inflate. Focus further investigation on measured hotpaths and real pixel-art workloads rather than on synthetic overlap patterns alone.
