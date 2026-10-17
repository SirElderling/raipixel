"""Measure an independently implemented indexed PNG frequency-ordered palette candidate.

Temporary safe-Rust source copy changes only the explicit opt-in indexed
writer. It keeps the production writer and fixed-Huffman DEFLATE unchanged.
The single-byte carry is retained across DEFLATE block boundaries and reset
at each PNG row. Round-trip, work charge, deterministic bytes and CRC
rejection are verified before native timing. No new dependencies, unsafe code
or reference-crate source inspection.

Usage: python tools/png_indexed_palette_frequency_trial.py INPUT.png [INPUT.png ...]
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
    # Counting occurs during the existing exact RGBA palette construction.
    indexed = indexed.replace(
        "    let mut slots = [u16::MAX; 4096];",
        "    let mut slots = [u16::MAX; 4096];\n    let mut frequencies = [0usize; 256];",
        1,
    )
    indexed = indexed.replace(
        "                colors.push(sample);",
        "                colors.push(sample);\n                frequencies[colors.len() - 1] = 1;",
        1,
    )
    original = "            if colors[usize::from(value)] == sample {\n                break;\n            }"
    if indexed.count(original) != 1:
        raise ValueError("frequency anchor changed")
    indexed = indexed.replace(original,
        "            if colors[usize::from(value)] == sample {\n"
        "                frequencies[usize::from(value)] += 1;\n"
        "                break;\n            }")
    marker = "    let width = image.width as usize;"
    if indexed.count(marker) != 1:
        raise ValueError("width anchor changed")
    reorder = """    // Scratch candidate: most frequent exact RGBA color first, stable ties
    // retain the original first-occurrence order. Rebuild the bounded lookup.
    let mut sorted: Vec<([u8; 4], usize)> = colors.iter().copied()
        .enumerate().map(|(i, rgba)| (rgba, frequencies[i])).collect();
    sorted.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    colors.clear();
    slots.fill(u16::MAX);
    for (sample, _) in sorted {
        let mut slot = slot_for(sample);
        while slots[slot] != u16::MAX {
            slot = (slot + 1) & 4095;
        }
        slots[slot] = colors.len() as u16;
        colors.push(sample);
    }

"""
    indexed = indexed.replace(marker, reorder + marker)
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
                   "palette remapping changed RGBA pixels");
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
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"accepted_bytes\":{},\"frequency_bytes\":{},\"samples_us\":{:?}}}",
                 width, height, accepted.len(), proposed.len(), samples);
    }
    Ok(())
}
'''

with tempfile.TemporaryDirectory(prefix="raipixel-indexed-palette-frequency-") as temp:
    folder = Path(temp)
    scratch = folder / "candidate"
    shutil.copytree(ROOT / "src", scratch / "src")
    changed = modified(SOURCE)
    (scratch / "src/png.rs").write_text(changed, encoding="utf-8")
    (scratch / "Cargo.toml").write_text(
        '[package]\nname="raipixel_palette_candidate"\nversion="0.0.0"\nedition="2024"\n'
        '[lib]\npath="src/lib.rs"\n', encoding="utf-8")
    (folder / "src").mkdir()
    (folder / "Cargo.toml").write_text(
        '[package]\nname="raipixel-indexed-palette-frequency-screen"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n'
        'candidate={package="raipixel_palette_candidate",path=' + json.dumps(str(scratch)) + '}\n',
        encoding="utf-8")
    (folder / "src/main.rs").write_text(DRIVER, encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path",
                    str(folder / "Cargo.toml")], check=True)
    exe = folder / "target/release" / (
        "raipixel-indexed-palette-frequency-screen.exe" if sys.platform == "win32" else "raipixel-indexed-palette-frequency-screen")
    lines = subprocess.check_output([str(exe), *sys.argv[1:]], text=True).splitlines()
    if len(lines) != len(sys.argv[1:]):
        raise ValueError("lost indexed palette frequency benchmark cases")
    cases = []
    for path, line in zip(sys.argv[1:], lines):
        case = json.loads(line)
        if case["supported"]:
            medians = [sorted(times)[len(times) // 2] for times in case["samples_us"]]
            case.update(median_us=medians,
                        size_over_original=case["frequency_bytes"] / case["accepted_bytes"],
                        time_over_original=medians[1] / medians[0])
        case.update(input=path, input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(case)
    print(json.dumps(dict(experiment="indexed_palette_frequency_filter_scratch_native",
                          platform=platform.platform(),
                          compiler=subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
                          source_sha256=hashlib.sha256(SOURCE.encode()).hexdigest(),
                          candidate_sha256=hashlib.sha256(changed.encode()).hexdigest(),
                          driver_sha256=hashlib.sha256(DRIVER.encode()).hexdigest(),
                          rounds=9, minimum_batch_ms=25, all_exact_rgba_crc_work_determinism_passed=True,
                          production_changed=False, cases=cases)))
