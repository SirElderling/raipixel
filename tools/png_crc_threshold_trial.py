"""Screen the data-size threshold for CRC's existing wide slicing loop.

The isolated variants use only our own safe Rust checksum.rs and preserve
its tables, polynomial, and architecture-specific slicing widths. No third-
party PNG implementation or dependencies are accessed. Production unchanged.

Usage: python tools/png_crc_threshold_trial.py
"""
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "src/checksum.rs").read_text(encoding="utf-8")
ANCHOR = "if part.len() >= 4096 {"
assert SOURCE.count(ANCHOR) == 2
THRESHOLDS = [0, 256, 1024, 4096, 16384]
NAMES = ["t0", "t256", "t1024", "t4096", "t16384"]
with tempfile.TemporaryDirectory(prefix="raipixel-crc-threshold-") as path:
    folder = Path(path)
    variants = {}
    for threshold, name in zip(THRESHOLDS, NAMES):
        text = SOURCE.replace(ANCHOR, f"if part.len() >= {threshold} {{")
        (folder / (name + ".rs")).write_text(text, encoding="utf-8")
        variants[name] = hashlib.sha256(text.encode()).hexdigest()
    mods = "\n".join(f'#[path = "{name}.rs"] mod {name};' for name in NAMES)
    driver = "#![forbid(unsafe_code)]\n#![allow(dead_code)]\n" + mods + r'''
use std::{hint::black_box, time::{Duration, Instant}};
fn bitwise(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
        }
    }
    !crc
}
fn main() {
    let functions: [fn(&[&[u8]]) -> u32; 5] = [
        t0::crc32, t256::crc32, t1024::crc32, t4096::crc32, t16384::crc32
    ];
    let mut state = 12345u32;
    let data: Vec<u8> = (0..1024 * 1024).map(|_| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as u8
    }).collect();
    for len in (0..=130).chain([
        255, 256, 257, 511, 512, 513, 1023, 1024, 1025,
        4095, 4096, 4097, 16383, 16384, 16385, 65535, 65536, 65537
    ]) {
        let correct = bitwise(&data[..len]);
        for split in [0, 1, 7, 8, 31, 32, 63, 64, len / 2, len] {
            if split <= len {
                for f in functions {
                    assert_eq!(f(&[&data[..split], &[], &data[split..len]]),
                        correct, "len {len}, split {split}");
                }
            }
        }
    }
    for len in [128, 256, 512, 1024, 2048, 4096, 8192, 65536, 262144, 1048576] {
        let input = &data[..len];
        let batch = |mode: usize, count: usize| {
            let start = Instant::now();
            for _ in 0..count {
                black_box(functions[mode](&[black_box(input)]));
            }
            start.elapsed()
        };
        let mut counts = [1usize; 5];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count) < Duration::from_millis(25) {
                *count = count.checked_mul(2).unwrap();
            }
        }
        let mut samples: [Vec<f64>; 5] = std::array::from_fn(|_| Vec::new());
        for round in 0..9 {
            for offset in 0..5 {
                let mode = (round + offset) % 5;
                samples[mode].push(
                    batch(mode, counts[mode]).as_secs_f64() * 1e6 / counts[mode] as f64
                );
            }
        }
        println!("{{\"bytes\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",
            len, counts, samples);
    }
}
'''
    (folder / "main.rs").write_text(driver, encoding="utf-8")
    executable = folder / ("crc-threshold.exe" if sys.platform == "win32" else "crc-threshold")
    subprocess.run(["rustc", "--edition=2024", "-O", str(folder / "main.rs"),
                    "-o", str(executable)], check=True)
    records = [json.loads(line) for line in
               subprocess.check_output([str(executable)], text=True).splitlines()]
    for item in records:
        medians = [sorted(v)[len(v)//2] for v in item["samples_us"]]
        item["median_us"] = medians
        item["relative_to_production"] = [v/medians[3] for v in medians]
    print(json.dumps({
        "experiment": "existing_crc_wide_slicing_thresholds",
        "threshold_bytes": THRESHOLDS,
        "comparison_base_bytes": 4096,
        "variants_sha256": variants,
        "compiler": subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
        "platform": platform.platform(),
        "rustup_toolchain": os.environ.get("RUSTUP_TOOLCHAIN", ""),
        "source_sha256": hashlib.sha256(SOURCE.encode()).hexdigest(),
        "rounds": 9,
        "minimum_batch_ms": 25,
        "cases": records
    }))
