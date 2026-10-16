"""Native screen of alternative XOR reduction groupings for raipixel CRC32.

Safe Rust, standard library only. Generates scratch copies of OUR checksum.rs;
the reference png crate is not inspected or used. The production CRC32 stays
unchanged unless a validated full-codec benefit is later demonstrated.

Run: python tools/png_crc_reduction_trial.py
"""
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "src/checksum.rs").read_text(encoding="utf-8")

def regroup(source, groups):
    # Keep the exact byte contributions, including the initial CRC state.
    # Change only the parentheses/order of XOR reductions in 32/64-byte loops.
    pattern = re.compile(r"(?m)^            crc = (CRC\[(?:31|63)\].*?);", re.DOTALL)
    found = 0
    def replace(match):
        nonlocal found
        expression = match.group(1)
        terms = expression.split("\n                ^ ")
        if len(terms) not in (32, 64):
            raise ValueError(f"Expected 32/64 contributions, got {len(terms)}")
        found += 1
        accumulators = [
            " ^ ".join(terms[stream::groups])
            for stream in range(groups)
        ]
        return (
            "\n".join(f"            let partial_{i} = {expr};" for i, expr in enumerate(accumulators))
            + "\n            crc = "
            + " ^ ".join(f"partial_{i}" for i in range(groups))
            + ";"
        )
    result = pattern.sub(replace, source)
    if found != 2:
        raise ValueError(f"Expected to change x86 and AArch64 widths, found {found}")
    return result

with tempfile.TemporaryDirectory(prefix="raipixel-crc-reduction-") as temporary:
    folder = Path(temporary)
    variants = {
        "original": SOURCE,
        "two_accumulators": regroup(SOURCE, 2),
        "four_accumulators": regroup(SOURCE, 4),
    }
    for label, source in variants.items():
        (folder / (label + ".rs")).write_text(source, encoding="utf-8")
    driver = r'''
#![forbid(unsafe_code)]
#![allow(dead_code)]
#[path = "original.rs"] mod original;
#[path = "two_accumulators.rs"] mod two_accumulators;
#[path = "four_accumulators.rs"] mod four_accumulators;
use std::{hint::black_box, time::{Duration, Instant}};

fn scalar(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    !crc
}
fn main() {
    let implementations: [fn(&[&[u8]]) -> u32; 3] = [
        original::crc32,
        two_accumulators::crc32,
        four_accumulators::crc32,
    ];
    let mut state = 19u32;
    let data: Vec<u8> = (0..4 * 1024 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        }).collect();

    // Independent bitwise polynomial oracle; exercise the large-slice
    // threshold, small tails, empty fragments, and block boundaries.
    for len in (0..=129).chain([
        255, 256, 257, 4095, 4096, 4097, 8191, 8192, 8193,
        65_535, 65_536, 65_537, 262_144
    ]) {
        let expected = scalar(&data[..len]);
        for split in [
            0, 1, 7, 8, 31, 32, 63, 64, len / 2, len
        ] {
            if split <= len {
                for f in implementations {
                    assert_eq!(f(&[&data[..split], &[], &data[split..len]]), expected,
                        "length {len}, split {split}");
                }
            }
        }
    }
    let test = [64, 4096, 262_144, 4 * 1024 * 1024];
    for len in test {
        let data = &data[..len];
        let batch = |mode: usize, count: usize| {
            let start = Instant::now();
            for _ in 0..count {
                black_box(implementations[mode](&[black_box(data)]));
            }
            start.elapsed()
        };
        let mut counts = [1usize; 3];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count) < Duration::from_millis(25) {
                *count = count.checked_mul(2).unwrap();
            }
        }
        let mut samples = [Vec::new(), Vec::new(), Vec::new()];
        for round in 0..9 {
            for offset in 0..3 {
                let mode = (round + offset) % 3;
                samples[mode].push(batch(mode, counts[mode]).as_secs_f64() * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"bytes\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",
            len, counts, samples);
    }
}
'''
    (folder / "main.rs").write_text(driver, encoding="utf-8")
    target = folder / ("crc-reduction.exe" if sys.platform == "win32" else "crc-reduction")
    subprocess.run(["rustc", "--edition=2024", "-O", str(folder / "main.rs"), "-o", str(target)], check=True)
    records = [json.loads(line) for line in subprocess.check_output([str(target)], text=True).splitlines()]
    for row in records:
        medians = [sorted(s)[len(s) // 2] for s in row["samples_us"]]
        row["median_us"] = medians
        row["relative_to_original"] = [v / medians[0] for v in medians]
    print(json.dumps({
        "experiment": "safe_crc_independent_xor_accumulator_groups",
        "variants": list(variants),
        "source_sha256": {k: hashlib.sha256(v.encode()).hexdigest() for k,v in variants.items()},
        "driver_sha256": hashlib.sha256(driver.encode()).hexdigest(),
        "compiler": subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
        "rustup_toolchain": os.environ.get("RUSTUP_TOOLCHAIN", ""),
        "platform": platform.platform(),
        "cases": records,
    }))
