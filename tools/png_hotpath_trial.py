"""Native, temporary-copy PNG decoder screens: CRC headroom and independent
Paeth variants. These are NOT production changes. No comparison-crate code is
read or compiled here. Only stdlib Python and safe, stable Rust are required.

Run after png_bench --export and restoration of png-original-inputs:
  python tools/png_hotpath_trial.py INPUT.png [INPUT.png ...]
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

def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f"Expected one source anchor: {old[:80]!r}")
    return source.replace(old, new)

with tempfile.TemporaryDirectory(prefix="raipixel-hotpath-") as temporary:
    folder = Path(temporary)
    packages = [
        ("trial_crc", "raipixel_trial_crc"),
        ("trial_paeth_math", "raipixel_trial_paeth_math"),
        ("trial_paeth_channels", "raipixel_trial_paeth_channels"),
    ]
    changes = {}
    for alias, package in packages:
        target = folder / alias
        shutil.copytree(ROOT / "src", target / "src")
        (target / "Cargo.toml").write_text(
            f'[package]\nname="{package}"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n',
            encoding="utf-8",
        )
        path = target / "src/png.rs"
        source = path.read_text(encoding="utf-8")
        if alias == "trial_crc":
            # INTENTIONALLY UNSAFE TO ACCEPT UNTRUSTED FILES: diagnostic only.
            # Remove only the CRC comparison, preserving framing/IDAT joins,
            # decoding, RGBA, resource charges and other validation.
            source = replace_once(
                source,
                'if crc32(&[&name, data]) != expected {',
                'if std::hint::black_box(false) {',
            )
        elif alias == "trial_paeth_math":
            old = '''    let p = a + b - c;
    let (da, db, dc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());'''
            new = '''    let (da, db, dc) = ((b - c).abs(), (a - c).abs(), (a + b - 2 * c).abs());'''
            source = replace_once(source, old, new)
        elif alias == "trial_paeth_channels":
            old = '''        *pixel = std::array::from_fn(|channel| {
            pixel[channel].wrapping_add(paeth(left[channel], up[channel], upper_left[channel]))
        });
        left = *pixel;
        upper_left = *up;'''
            new = '''        for channel in 0..4 {
            let reconstructed =
                pixel[channel].wrapping_add(paeth(left[channel], up[channel], upper_left[channel]));
            pixel[channel] = reconstructed;
            left[channel] = reconstructed;
            upper_left[channel] = up[channel];
        }'''
            source = replace_once(source, old, new)
        path.write_text(source, encoding="utf-8")
        changes[alias] = hashlib.sha256(source.encode()).hexdigest()

    manifest = '[package]\nname="raipixel-hotpath-trial"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
    manifest += 'raipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    for alias, package in packages:
        manifest += alias + '={package=' + json.dumps(package) + ',path=' + json.dumps(str(folder / alias)) + '}\n'
    (folder / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (folder / "src").mkdir()
    (folder / "src/main.rs").write_text(r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Instant, Duration}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path)?;
        let mut work = raipixel::Work::default();
        let baseline = raipixel::png::decode(&data, raipixel::Limits::default(), &mut work)?;
        let expected = baseline.image.pixels().as_flattened();
        let expected_work = work.remaining();
        {
            let mut w = trial_crc::Work::default();
            let trial = trial_crc::png::decode(&data, trial_crc::Limits::default(), &mut w)?;
            assert_eq!(trial.image.pixels().as_flattened(), expected);
            assert_eq!(trial.trailing_idat_bytes, baseline.trailing_idat_bytes);
            assert_eq!(w.remaining(), expected_work);
        }
        {
            let mut w = trial_paeth_math::Work::default();
            let trial = trial_paeth_math::png::decode(&data, trial_paeth_math::Limits::default(), &mut w)?;
            assert_eq!(trial.image.pixels().as_flattened(), expected);
            assert_eq!(trial.trailing_idat_bytes, baseline.trailing_idat_bytes);
            assert_eq!(w.remaining(), expected_work);
        }
        {
            let mut w = trial_paeth_channels::Work::default();
            let trial = trial_paeth_channels::png::decode(&data, trial_paeth_channels::Limits::default(), &mut w)?;
            assert_eq!(trial.image.pixels().as_flattened(), expected);
            assert_eq!(trial.trailing_idat_bytes, baseline.trailing_idat_bytes);
            assert_eq!(w.remaining(), expected_work);
        }
        // Prove that the no-CRC copy is genuinely invalid for production.
        // Corrupt the CRC of the otherwise valid IHDR chunk.
        let mut broken = data.clone();
        broken[29] ^= 1;
        assert!(matches!(
            raipixel::png::decode(&broken, raipixel::Limits::default(), &mut raipixel::Work::default()),
            Err(raipixel::png::Error::Crc)
        ));
        assert!(trial_crc::png::decode(
            &broken, trial_crc::Limits::default(), &mut trial_crc::Work::default()
        ).is_ok());

        let batch = |mode: usize, count: usize| -> Result<Duration, Box<dyn std::error::Error>> {
            let start = Instant::now();
            for _ in 0..count {
                match mode {
                    0 => { black_box(raipixel::png::decode(
                        black_box(&data), raipixel::Limits::default(), &mut raipixel::Work::default()
                    )?); },
                    1 => { black_box(trial_crc::png::decode(
                        black_box(&data), trial_crc::Limits::default(), &mut trial_crc::Work::default()
                    )?); },
                    2 => { black_box(trial_paeth_math::png::decode(
                        black_box(&data), trial_paeth_math::Limits::default(), &mut trial_paeth_math::Work::default()
                    )?); },
                    3 => { black_box(trial_paeth_channels::png::decode(
                        black_box(&data), trial_paeth_channels::Limits::default(), &mut trial_paeth_channels::Work::default()
                    )?); },
                    _ => unreachable!(),
                }
            }
            Ok(start.elapsed())
        };
        let mut counts = [1usize; 4];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count)? < Duration::from_millis(25) {
                *count = count.checked_mul(2).ok_or("benchmark batch overflow")?;
            }
        }
        let mut samples = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for round in 0..9 {
            for n in 0..4 {
                let mode = (round + n) % 4;
                samples[mode].push(batch(mode, counts[mode])?.as_secs_f64() * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"width\":{},\"height\":{},\"bytes\":{},\"counts\":{:?},\"samples_us\":{:?}}}",
            baseline.image.size()[0], baseline.image.size()[1], data.len(), counts, samples);
    }
    Ok(())
}
''', encoding="utf-8")
    subprocess.run(
        ["cargo", "build", "--release", "--offline", "--manifest-path", str(folder / "Cargo.toml")],
        check=True,
    )
    executable = folder / "target/release" / ("raipixel-hotpath-trial.exe" if sys.platform == "win32" else "raipixel-hotpath-trial")
    lines = subprocess.check_output([str(executable), *sys.argv[1:]], text=True).splitlines()
    if len(lines) != len(sys.argv[1:]):
        raise ValueError("Unexpected number of native benchmark outputs")
    cases = []
    for path, line in zip(sys.argv[1:], lines):
        row = json.loads(line)
        medians = []
        for samples in row["samples_us"]:
            medians.append(sorted(samples)[len(samples)//2])
        row.update(input=path, median_us=medians,
                   relative_to_baseline=[x / medians[0] for x in medians],
                   input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(row)
    print(json.dumps({
        "experiment": "isolated_crc_headroom_and_safe_paeth_screens",
        "warning": "trial_crc intentionally skips PNG chunk CRC validation; diagnostic upper bound only, NEVER for production",
        "modes": ["baseline", "crc_validation_skipped_only_in_scratch", "paeth_delta_math", "paeth_per_channel"],
        "arch": platform.machine(),
        "os": platform.platform(),
        "compiler": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "source_sha256": changes,
        "minimum_batch_ms": 25,
        "rounds": 9,
        "cases": cases,
    }))
