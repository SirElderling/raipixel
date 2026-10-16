# Architecture-aware safe CRC32 slicing — 16 October 2026

## Decision

Retain a small, compile-time architecture-specific CRC policy, not OS-specific
forks: x86-64 uses the already accepted 32-byte slicing implementation, AArch64
uses 64-byte slicing, and other targets use the portable eight-byte loop.
The public/internal `crc32(parts)` interface and PNG byte output do not change.
No intrinsics, unsafe code, new dependencies, runtime dispatch, or third-party
implementation inspection are involved.

The extra 32 tables on AArch64 occupy **32 KiB of additional static read-only
data** (64 KiB total). x86-64 still uses 32 KiB, and other targets use 8 KiB.
All paths use the independently implemented reflected PNG CRC-32 polynomial;
small chunks and large-chunk remainders use the original eight-byte path.

## Why specialize by architecture?

The standalone native CRC screen
(`tools/png_crc_trial.py`, run
[38814936217](https://github.com/SirElderling/raipixel/actions/runs/38814936217))
tested 8, 16, 32 and 64 slicing widths under one Rust compiler/process,
with nine rotating calibrated batches. Each implementation agreed with an
independent bitwise polynomial oracle at varied sizes and fragment boundaries.
For 256 KiB inputs, the median timings were:

| Native runner | 32-byte, µs | 64-byte, µs | 64 / 32 |
|---|---:|---:|---:|
| Linux x86-64 | 73.09 | 94.36 | 1.291 |
| Windows x86-64 | 72.47 | 95.76 | 1.322 |
| macOS ARM64 | 120.83 | 80.82 | 0.669 |

This supports **CPU architecture selection**, not selecting based on the OS
name. Hardware, compiler, runner conditions and payload sizes still matter.
The faster ARM64 microbenchmark is a reason to test 64-byte slicing, not
sufficient proof of a whole-codec advantage.

## Isolated complete-codec comparison

Native CI directly compared the immediately preceding production revision
`9734e8102eb3624fb7324561e32031112c0e838c` (32-byte slicing on all
targets) with the architecture-aware candidate
`ae1182f2d1df4c44eac4464095503ee44be1d130`.
The preceding encoder fast path is **identical in both builds**. The driver,
compiler, output verification and build profile match; each test uses the
paired before/after process order and a 25-ms minimum per batch. The reported
times include complete PNG decoding and allocation, not just the CRC stage.

| Decode workload | Linux x86-64 (new/old) | Windows x86-64 | macOS ARM64 |
|---|---:|---:|---:|
| Generated noise 256 | 1.005 | 1.054 | **0.823** |
| Sheet 1024 | 0.992 | 0.997 | 1.119 |
| Texture 1024 | 1.000 | 0.999 | 1.136 |
| Icon 16 | 0.994 | 0.995 | 1.290 |
| Icon 64 | 0.999 | 1.017 | 1.154 |

These figures are from [native run 38850769461](https://github.com/SirElderling/raipixel/actions/runs/38850769461).
All Linux, Windows and ARM64 macOS jobs passed; exact PNG output and work
accounting matched. The ARM64 noise decode measurement improved from 217.82
to 179.20 µs. However, the 16-pixel icon (which cannot enter the >=4096-byte
wide CRC path) also varied by 29% between supposedly equivalent runs.
Similarly, Windows noise varied by 5% despite no change to its x86-64 CRC
algorithm. This shows substantial native-run noise. **Do not attribute the
full macOS sheet, texture or icon differences to CRC** or declare full-codec
parity on this evidence. The effect is most compelling for large, CRC-heavy,
incompressible payloads, corroborated by the standalone CRC screen.

## Validation and limits

- `cargo fmt --check`, native Rust tests, Clippy and all other CI stages
  passed on Linux x86-64, Windows x86-64, and macOS AArch64 in
  [run 38850769461](https://github.com/SirElderling/raipixel/actions/runs/38850769461).
- The added test compares every branch against the bitwise polynomial at
  lengths around 8/32/64 bytes, 4096 bytes, 8192 bytes and 65535 bytes,
  including fragmentation. Existing corruption, checksum, work-limit,
  reference-decoder, and exact-encoded-output checks also ran.
- The portable eight-byte fallback is exercised mathematically by the same
  CRC recurrence, but has **not yet been compiled on a third CPU architecture
  in this native CI matrix**.
- This is a modest, isolated architecture choice, **not** an invitation to
  fork the whole PNG pipeline by operating system. More changes require
  controlled native evidence and a meaningful whole-codec win.

The independent slicing method attribution remains in
[algorithm notes](../algorithm-notes.md). The comparison Rust `png` crate
was used only through its public API.
