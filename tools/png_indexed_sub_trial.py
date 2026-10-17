"""Measure an independently implemented indexed PNG Sub-filter candidate.

Temporary safe-Rust source copy changes only the explicit opt-in indexed
writer. It keeps the production writer and fixed-Huffman DEFLATE unchanged.
The single-byte carry is retained across DEFLATE block boundaries and reset
at each PNG row. Round-trip, work charge, deterministic bytes and CRC
rejection are verified before native timing. No new dependencies, unsafe code
or reference-crate source inspection.

Usage: python tools/png_indexed_sub_trial.py INPUT.png [INPUT.png ...]
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
SOURCE = (ROOT / "src/png.rs").read_text(encoding="utf-8")


def modified(source):
    before, separator, rest = source.partition("pub fn encode_indexed(")
    if not separator:
        raise ValueError("indexed writer missing")
    indexed, separator, after = rest.partition("\nfn encode_filtered<")
    if not separator:
        raise ValueError("indexed writer end missing")
    start = "    while offset < raw_size {"
    anchor = "        adler.update(&raw);"
    if indexed.count(start) != 1 or indexed.count(anchor) != 1:
        raise ValueError("indexed writer anchors changed")
    indexed = indexed.replace(start, "    let mut sub_previous = 0u8;\n" + start)
    indexed = indexed.replace(
        anchor,
        """        // Scratch only: PNG Sub predictor uses the previous *unfiltered*
        // packed byte. The carry must survive a 65,535-byte DEFLATE block
        // split and reset only at an actual scanline filter marker.
        for (i, byte) in raw.iter_mut().enumerate() {
            if (offset + i) % stride == 0 {
                *byte = 1;
                sub_previous = 0;
            } else {
                let original = *byte;
                *byte = original.wrapping_sub(sub_previous);
                sub_previous = original;
            }
        }
""" + anchor)
    return before + "pub fn encode_indexed(" + indexed + separator + after


DRIVER = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Duration, Instant}};
type Failure = Box<dyn std::error::Error>;

fn main() -> Result<(), Failure> {
    for path in std::env::args().skip(1) {
        let input = std::fs::read(&path)?;
        let base = raipixel::png::decode(&input, raipixel::Limits::default(),
                                        &mut raipixel::Work::default())?.image;
        let [width, height] = base.size();
        let trial = candidate::Image::from_pixels(width, height, base.pixels(),
                      candidate::Limits::default(), &mut candidate::Work::default())?;
        let mut accepted = Vec::new();
        let mut bw = raipixel::Work::default();
        let supported = raipixel::png::encode_indexed(&base, &mut accepted, &mut bw)?;
        let mut proposed = Vec::new();
        let mut tw = candidate::Work::default();
        let candidate_supported = candidate::png::encode_indexed(&trial, &mut proposed, &mut tw)?;
        assert_eq!(supported, candidate_supported, "supported palette mismatch");
        assert_eq!(bw.remaining(), tw.remaining(), "changed Work accounting");
        if !supported {
            assert!(accepted.is_empty() && proposed.is_empty(), "unsupported image wrote bytes");
            println!("{{\"supported\":false,\"width\":{},\"height\":{}}}", width, height);
            continue;
        }
        assert_eq!(raipixel::png::decode(&proposed, raipixel::Limits::default(),
                       &mut raipixel::Work::default())?.image, base,
                   "candidate changed any RGBA pixel");
        let mut repeated = Vec::new();
        candidate::png::encode_indexed(&trial, &mut repeated,
                                      &mut candidate::Work::default())?;
        assert_eq!(proposed, repeated, "candidate not deterministic");
        let mut corrupt = proposed.clone();
        corrupt[29] ^= 1;
        assert!(matches!(raipixel::png::decode(&corrupt, raipixel::Limits::default(),
                        &mut raipixel::Work::default()), Err(raipixel::png::Error::Crc)),
                "corrupted IHDR CRC accepted");

        let batch = |mode: usize, count: usize| -> Result<Duration, Failure> {
            let started = Instant::now();
            for _ in 0..count {
                let mut output = Vec::new();
                if mode == 0 {
                    raipixel::png::encode_indexed(black_box(&base), &mut output,
                                                 &mut raipixel::Work::default())?;
                } else {
                    candidate::png::encode_indexed(black_box(&trial), &mut output,
                                                   &mut candidate::Work::default())?;
                }
                black_box(output);
            }
            Ok(started.elapsed())
        };
        let mut counts = [1usize; 2];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count)? < Duration::from_millis(25) {
                *count = count.checked_mul(2).ok_or("iteration overflow")?;
            }
        }
        let mut samples = [Vec::new(), Vec::new()];
        for round in 0..9 {
            for mode in [round % 2, (round + 1) % 2] {
                samples[mode].push(batch(mode, counts[mode])?.as_secs_f64()
                                    * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"accepted_bytes\":{},\"sub_bytes\":{},\"samples_us\":{:?}}}",
                 width, height, accepted.len(), proposed.len(), samples);
    }
    Ok(())
}
'''

with tempfile.TemporaryDirectory(prefix="raipixel-indexed-sub-") as temp:
    folder = Path(temp)
    scratch = folder / "candidate"
    shutil.copytree(ROOT / "src", scratch / "src")
    changed = modified(SOURCE)
    (scratch / "src/png.rs").write_text(changed, encoding="utf-8")
    (scratch / "Cargo.toml").write_text(
        '[package]\nname="raipixel_sub_candidate"\nversion="0.0.0"\nedition="2024"\n'
        '[lib]\npath="src/lib.rs"\n', encoding="utf-8")
    (folder / "src").mkdir()
    (folder / "Cargo.toml").write_text(
        '[package]\nname="raipixel-indexed-sub-screen"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n'
        'candidate={package="raipixel_sub_candidate",path=' + json.dumps(str(scratch)) + '}\n',
        encoding="utf-8")
    (folder / "src/main.rs").write_text(DRIVER, encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path",
                    str(folder / "Cargo.toml")], check=True)
    exe = folder / "target/release" / (
        "raipixel-indexed-sub-screen.exe" if sys.platform == "win32" else "raipixel-indexed-sub-screen")
    lines = subprocess.check_output([str(exe), *sys.argv[1:]], text=True).splitlines()
    if len(lines) != len(sys.argv[1:]):
        raise ValueError("lost indexed Sub benchmark cases")
    cases = []
    for path, line in zip(sys.argv[1:], lines):
        case = json.loads(line)
        if case["supported"]:
            medians = [sorted(times)[len(times) // 2] for times in case["samples_us"]]
            case.update(median_us=medians,
                        size_over_none=case["sub_bytes"] / case["accepted_bytes"],
                        time_over_none=medians[1] / medians[0])
        case.update(input=path, input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(case)
    print(json.dumps(dict(experiment="indexed_sub_filter_scratch_native",
                          platform=platform.platform(),
                          compiler=subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
                          source_sha256=hashlib.sha256(SOURCE.encode()).hexdigest(),
                          candidate_sha256=hashlib.sha256(changed.encode()).hexdigest(),
                          driver_sha256=hashlib.sha256(DRIVER.encode()).hexdigest(),
                          rounds=9, minimum_batch_ms=25, all_exact_rgba_crc_work_determinism_passed=True,
                          production_changed=False, cases=cases)))
