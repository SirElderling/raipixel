"""Isolated direct CRC32 throughput comparison against crc32fast's PUBLIC API.

The reference library is temporary benchmark-only, never a raipixel runtime
dependency. Its implementation is NOT inspected. The accepted CRC source is
our own src/checksum.rs, compiled under the identical Rust 1.99.1 release
profile. Independent polynomial oracle, hash equality, and fragment checks.
Usage: python tools/png_crc_reference_api.py
"""
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DRIVER = r'''
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "checksum.rs"]
mod checksum;
use std::{hint::black_box, time::{Duration, Instant}};
fn scalar(data: &[u8]) -> u32 {
    let mut state = !0u32;
    for &byte in data {
        state ^= u32::from(byte);
        for _ in 0..8 {
            state = (state >> 1) ^ if state & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    !state
}
fn main() {
    let mut seed = 0x93ed_8c17u32;
    let payload: Vec<u8> = (0..4 * 1024 * 1024)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 16) as u8
        }).collect();
    let parts: [&[u8]; 2] = [b"IDAT", &payload];
    assert_eq!(checksum::crc32(&[b"123456789"]), 0xcbf4_3926);
    for len in (0..=128).chain([255,256,257,4095,4096,4097,65535,65536,262144]) {
        let data = &payload[..len];
        let expected = scalar(data);
        for split in [0, 1, 2, 7, 8, 16, 31, 32, 63, 64, len/2, len] {
            if split <= len {
                let parts = [&data[..split], &data[split..]];
                let mut hasher = crc32fast::Hasher::new();
                hasher.update(parts[0]);
                hasher.update(parts[1]);
                assert_eq!(hasher.finalize(), expected);
                assert_eq!(checksum::crc32(&parts), expected);
            }
        }
    }
    for count in [256usize, 4096, 65536, 262144, 4 * 1024 * 1024] {
        let slices: [&[u8]; 2] = [parts[0], &parts[1][..count]];
        let mut expected = crc32fast::Hasher::new();
        expected.update(slices[0]);
        expected.update(slices[1]);
        assert_eq!(checksum::crc32(&slices), expected.finalize());
        let batch = |mode: usize, count_iters: usize| {
            let start = Instant::now();
            for _ in 0..count_iters {
                let result = if mode == 0 {
                    checksum::crc32(black_box(&slices))
                } else {
                    let mut hasher = crc32fast::Hasher::new();
                    hasher.update(black_box(slices[0]));
                    hasher.update(black_box(slices[1]));
                    hasher.finalize()
                };
                black_box(result);
            }
            start.elapsed()
        };
        let mut iters = [1usize; 2];
        for (mode, n) in iters.iter_mut().enumerate() {
            while batch(mode, *n) < Duration::from_millis(25) {
                *n = n.checked_mul(2).expect("benchmark overflow");
            }
        }
        let mut samples: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::new());
        for round in 0..9 {
            for mode in [round % 2, (round + 1) % 2] {
                samples[mode].push(batch(mode, iters[mode]).as_secs_f64() * 1e6 / iters[mode] as f64);
            }
        }
        println!("{{\"bytes\":{},\"samples_us\":{:?}}}", count, samples);
    }
}
'''
with tempfile.TemporaryDirectory(prefix="raipixel-crc-api-") as temporary:
    root = Path(temporary)
    (root / "src").mkdir()
    # Same locked reference version as our external png 0.18.1 tests.
    shutil.copyfile(ROOT / "benchmarks/png/reference.lock", root / "Cargo.lock")
    shutil.copyfile(ROOT / "src/checksum.rs", root / "src/checksum.rs")
    (root / "src/main.rs").write_text(DRIVER, encoding="utf-8")
    (root / "Cargo.toml").write_text(
        '[package]\nname="png-comparison"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\npng="=0.18.1"\ncrc32fast="=1.5.2"\n'
        'raipixel={path=' + json.dumps(str(ROOT)) + '}\n',
        encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline",
                    "--manifest-path", str(root / "Cargo.toml")], check=True)
    binary = root / "target/release" / ("png-comparison.exe" if sys.platform == "win32" else "png-comparison")
    cases = [json.loads(line) for line in
             subprocess.check_output([str(binary)], text=True).splitlines()]
    for case in cases:
        medians = [sorted(values)[len(values)//2] for values in case["samples_us"]]
        case["median_us"] = medians
        case["raipixel_over_reference"] = medians[0] / medians[1]
    print(json.dumps({
        "experiment": "png_crc32_against_crc32fast_public_api_only",
        "reference_version": "crc32fast 1.5.2 as locked by png 0.18.1",
        "dependency_scope": "disposable diagnostic package only",
        "compiler": subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
        "platform": platform.platform(),
        "source_sha256": hashlib.sha256((ROOT / "src/checksum.rs").read_bytes()).hexdigest(),
        "driver_sha256": hashlib.sha256(DRIVER.encode()).hexdigest(),
        "minimum_batch_ms": 25,
        "rounds": 9,
        "cases": cases
    }))
