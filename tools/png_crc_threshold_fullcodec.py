"""Test whether an earlier CRC-wide-slicing threshold speeds actual PNG decode.

Compares the unchanged raipixel PNG decoder with temporary copies differing
ONLY in the CRC wide-slicing threshold (0 or 1024 rather than 4096 bytes).
These copies retain ALL PNG CRC validation. No reference-crate implementation,
third-party dependencies or unsafe Rust. Work/pixels/flags/CRC rejection checked.
Run after exporting png-stage-inputs and restoring png-original-inputs:
  python tools/png_crc_threshold_fullcodec.py INPUT.png [INPUT.png ...]
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
SOURCE = (ROOT / "src/checksum.rs").read_text(encoding="utf-8")
ANCHOR = "if part.len() >= 4096 {"
assert SOURCE.count(ANCHOR) == 2, "Unexpected production CRC implementation"

with tempfile.TemporaryDirectory(prefix="raipixel-crc-codec-") as temporary:
    folder = Path(temporary)
    packages = [("crc_early", "raipixel_crc_early", 0),
                ("crc_middle", "raipixel_crc_middle", 1024)]
    hashes = {}
    for alias, package, threshold in packages:
        directory = folder / alias
        shutil.copytree(ROOT / "src", directory / "src")
        edited = SOURCE.replace(ANCHOR, f"if part.len() >= {threshold} {{")
        (directory / "src/checksum.rs").write_text(edited, encoding="utf-8")
        (directory / "Cargo.toml").write_text(
            f'[package]\nname="{package}"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n',
            encoding="utf-8",
        )
        hashes[alias] = hashlib.sha256(edited.encode()).hexdigest()

    manifest = ('[package]\nname="raipixel-crc-fullcodec"\n'
                'version="0.0.0"\nedition="2024"\n[dependencies]\n')
    manifest += 'raipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    for alias, package, _ in packages:
        manifest += (alias + '={package=' + json.dumps(package) +
                     ',path=' + json.dumps(str(folder / alias)) + '}\n')
    (folder / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (folder / "src").mkdir()
    driver = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Duration, Instant}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args().skip(1) {
        let input = std::fs::read(&path)?;
        let mut base_work = raipixel::Work::default();
        let base = raipixel::png::decode(&input, raipixel::Limits::default(), &mut base_work)?;
        let mut early_work = crc_early::Work::default();
        let early = crc_early::png::decode(&input, crc_early::Limits::default(), &mut early_work)?;
        let mut middle_work = crc_middle::Work::default();
        let middle = crc_middle::png::decode(&input, crc_middle::Limits::default(), &mut middle_work)?;
        for (pixels, work, flags) in [
            (early.image.pixels().as_flattened(), early_work.remaining(),
             (early.uninterpreted_color, early.skipped_ancillary, early.trailing_idat_bytes)),
            (middle.image.pixels().as_flattened(), middle_work.remaining(),
             (middle.uninterpreted_color, middle.skipped_ancillary, middle.trailing_idat_bytes))
        ] {
            assert_eq!(pixels, base.image.pixels().as_flattened());
            assert_eq!(work, base_work.remaining());
            assert_eq!(flags, (base.uninterpreted_color, base.skipped_ancillary, base.trailing_idat_bytes));
        }
        let mut bad = input.clone();
        bad[29] ^= 1; // IHDR CRC; all variants MUST reject bad checksums.
        assert!(matches!(raipixel::png::decode(&bad, raipixel::Limits::default(),
            &mut raipixel::Work::default()), Err(raipixel::png::Error::Crc)));
        assert!(matches!(crc_early::png::decode(&bad, crc_early::Limits::default(),
            &mut crc_early::Work::default()), Err(crc_early::png::Error::Crc)));
        assert!(matches!(crc_middle::png::decode(&bad, crc_middle::Limits::default(),
            &mut crc_middle::Work::default()), Err(crc_middle::png::Error::Crc)));
        let batch = |mode: usize, count: usize| -> Result<Duration, Box<dyn std::error::Error>> {
            let start = Instant::now();
            for _ in 0..count {
                match mode {
                    0 => { black_box(raipixel::png::decode(black_box(&input),
                            raipixel::Limits::default(), &mut raipixel::Work::default())?); }
                    1 => { black_box(crc_early::png::decode(black_box(&input),
                            crc_early::Limits::default(), &mut crc_early::Work::default())?); }
                    2 => { black_box(crc_middle::png::decode(black_box(&input),
                            crc_middle::Limits::default(), &mut crc_middle::Work::default())?); }
                    _ => unreachable!()
                }
            }
            Ok(start.elapsed())
        };
        let mut counts = [1usize; 3];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count)? < Duration::from_millis(25) {
                *count = count.checked_mul(2).ok_or("benchmark iteration overflow")?;
            }
        }
        let mut samples = [Vec::new(), Vec::new(), Vec::new()];
        for round in 0..9 {
            for offset in 0..3 {
                let mode = (round + offset) % 3;
                samples[mode].push(batch(mode, counts[mode])?.as_secs_f64() * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"bytes\":{},\"width\":{},\"height\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",
            input.len(), base.image.size()[0], base.image.size()[1], counts, samples);
    }
    Ok(())
}
'''
    (folder / "src/main.rs").write_text(driver, encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path",
                    str(folder / "Cargo.toml")], check=True)
    exe = folder / "target/release" / (
        "raipixel-crc-fullcodec.exe" if sys.platform == "win32" else "raipixel-crc-fullcodec"
    )
    lines = subprocess.check_output([str(exe), *sys.argv[1:]], text=True).splitlines()
    if len(lines) != len(sys.argv[1:]):
        raise ValueError("Incomplete benchmark output")
    cases = []
    for path, line in zip(sys.argv[1:], lines):
        case = json.loads(line)
        medians = [sorted(x)[len(x) // 2] for x in case["samples_us"]]
        case.update(input=path, median_us=medians,
                    relative_to_baseline=[x / medians[0] for x in medians],
                    input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(case)
    print(json.dumps({
        "experiment": "safe_crc_threshold_complete_png_decode",
        "thresholds": [4096, 0, 1024],
        "all_modes_validate_chunk_crc": True,
        "exact_output_flags_and_work_verified": True,
        "compiler": subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
        "platform": platform.platform(),
        "rustup_toolchain": os.environ.get("RUSTUP_TOOLCHAIN", ""),
        "source_sha256": hashlib.sha256(SOURCE.encode()).hexdigest(),
        "variants_sha256": hashes,
        "driver_sha256": hashlib.sha256(driver.encode()).hexdigest(),
        "minimum_batch_ms": 25,
        "rounds": 9,
        "cases": cases
    }))
