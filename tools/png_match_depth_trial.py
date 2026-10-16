"""Scratch screen of more thorough LZ match selection, same fixed Huffman.

Tests current one-entry hash, one-token lazy lookahead, two-entry collision
history, and combined. Independently validates full PNG RGBA roundtrip with the
accepted decoder; reports output size AND whole compressed-writer runtime.
Uses only OUR encoder code, Rust 1.99.1, safe stable Rust, no new dependencies.
Production unchanged. Run after PNG export and original-input restoration:
    python tools/png_match_depth_trial.py INPUT.png [INPUT.png ...]
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
SRC = (ROOT / "src/deflate/encode.rs").read_text(encoding="utf-8")

def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError("Unexpected encoder anchor: " + repr(old[:80]))
    return source.replace(old, new)

def candidate(two_way, lazy):
    s = SRC
    if two_way:
        s = replace_once(s, "try_reserve_exact(HASH_SIZE)", "try_reserve_exact(HASH_SIZE * 2)")
        s = replace_once(s, "dictionary.resize(HASH_SIZE, u32::MAX);",
                         "dictionary.resize(HASH_SIZE * 2, u32::MAX);")
        s = replace_once(s, """            let entry = self.dictionary[slot];
            let stored = (entry & 65535) as usize;
            self.dictionary[slot] = (prefix << 16) | pos as u32;""",
                         """            let entry = self.dictionary[slot];
            let second = self.dictionary[slot + HASH_SIZE];
            let stored = (entry & 65535) as usize;
            self.dictionary[slot + HASH_SIZE] = entry;
            self.dictionary[slot] = (prefix << 16) | pos as u32;""")
        s = replace_once(s, """            if length >= 3 {
                self.reference(length, pos - prior);""",
                         """            // A prior collision might still be a longer match than the
            // immediately preceding dictionary entry.
            let alternative = match_candidate(data, second, prefix, pos, maximum);
            let (prior, length) = if alternative > length {
                ((second & 65535) as usize, alternative)
            } else {
                (prior, length)
            };
            if length >= 3 {
                self.reference(length, pos - prior);""")
    if lazy:
        addon = """            // One-step lazy parse: prefer a longer match starting after one
            // literal, with no changes to DEFLATE coding or window bounds.
            if length >= 3 && length < 258 && pos + 4 <= data.len() {
                let next_pos = pos + 1;
                let (next_slot, next_prefix) = hash(data, next_pos);
                let next_maximum = (data.len() - next_pos).min(258);
                let next_match = match_candidate(
                    data, self.dictionary[next_slot], next_prefix, next_pos, next_maximum
                );
                NEXT_MATCH
                if next_match > length + 1 {
                    self.symbol(u32::from(data[pos]));
                    pos += 1;
                    continue;
                }
            }
"""
        if two_way:
            addon=addon.replace("NEXT_MATCH", """let next_alternative = match_candidate(
                    data, self.dictionary[next_slot + HASH_SIZE], next_prefix, next_pos, next_maximum
                );
                let next_match = next_match.max(next_alternative);""")
        else:
            addon=addon.replace("                NEXT_MATCH\n", "")
        s = replace_once(s, """            if length >= 3 {
                self.reference(length, pos - prior);""",
                         addon + """            if length >= 3 {
                self.reference(length, pos - prior);""")
    return s

with tempfile.TemporaryDirectory(prefix="raipixel-match-depth-") as temporary:
    folder = Path(temporary)
    specs = [("lazy", "raipixel_lazy", False, True),
             ("depth2", "raipixel_depth2", True, False),
             ("both", "raipixel_both", True, True)]
    fingerprints = {}
    for alias, package, depth, lazy in specs:
        root = folder / alias
        shutil.copytree(ROOT / "src", root / "src")
        text = candidate(depth, lazy)
        (root / "src/deflate/encode.rs").write_text(text, encoding="utf-8")
        (root / "Cargo.toml").write_text(
            f'[package]\nname="{package}"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n', encoding="utf-8")
        fingerprints[alias] = hashlib.sha256(text.encode()).hexdigest()
    manifest = '[package]\nname="raipixel-match-depth"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
    manifest += 'raipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    for alias, package, _, _ in specs:
        manifest += alias + '={package=' + json.dumps(package) + ',path=' + json.dumps(str(folder / alias)) + '}\n'
    (folder / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (folder / "src").mkdir()
    driver = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Duration, Instant}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args().skip(1) {
        let input = std::fs::read(&path)?;
        let img = raipixel::png::decode(&input, raipixel::Limits::default(), &mut raipixel::Work::default())?.image;
        let (w, h) = (img.size()[0], img.size()[1]);
        let a = lazy::Image::from_pixels(w, h, img.pixels(), lazy::Limits::default(), &mut lazy::Work::default())?;
        let b = depth2::Image::from_pixels(w, h, img.pixels(), depth2::Limits::default(), &mut depth2::Work::default())?;
        let c = both::Image::from_pixels(w, h, img.pixels(), both::Limits::default(), &mut both::Work::default())?;
        let mut blobs = Vec::new();
        for mode in 0..4 {
            let mut out = Vec::new();
            match mode {
                0 => raipixel::png::encode_compressed(&img, &mut out, &mut raipixel::Work::default())?,
                1 => lazy::png::encode_compressed(&a, &mut out, &mut lazy::Work::default())?,
                2 => depth2::png::encode_compressed(&b, &mut out, &mut depth2::Work::default())?,
                3 => both::png::encode_compressed(&c, &mut out, &mut both::Work::default())?,
                _ => unreachable!()
            }
            let decoded = raipixel::png::decode(&out, raipixel::Limits::default(), &mut raipixel::Work::default())?;
            assert_eq!(decoded.image.pixels(), img.pixels(), "PNG pixels change");
            let mut verify = Vec::new();
            match mode {
                0 => raipixel::png::encode_compressed(&img, &mut verify, &mut raipixel::Work::default())?,
                1 => lazy::png::encode_compressed(&a, &mut verify, &mut lazy::Work::default())?,
                2 => depth2::png::encode_compressed(&b, &mut verify, &mut depth2::Work::default())?,
                3 => both::png::encode_compressed(&c, &mut verify, &mut both::Work::default())?,
                _ => unreachable!()
            }
            assert_eq!(out, verify, "non-deterministic compressed output");
            blobs.push(out);
        }
        let batch = |mode: usize, count: usize| -> Result<Duration, Box<dyn std::error::Error>> {
            let start = Instant::now();
            for _ in 0..count {
                let mut out = Vec::new();
                match mode {
                    0 => raipixel::png::encode_compressed(black_box(&img), &mut out, &mut raipixel::Work::default())?,
                    1 => lazy::png::encode_compressed(black_box(&a), &mut out, &mut lazy::Work::default())?,
                    2 => depth2::png::encode_compressed(black_box(&b), &mut out, &mut depth2::Work::default())?,
                    3 => both::png::encode_compressed(black_box(&c), &mut out, &mut both::Work::default())?,
                    _ => unreachable!()
                }
                black_box(out);
            }
            Ok(start.elapsed())
        };
        let mut counts = [1usize; 4];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count)? < Duration::from_millis(25) {
                *count = count.checked_mul(2).ok_or("calibration overflow")?;
            }
        }
        let mut samples: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
        for round in 0..9 {
            for offset in 0..4 {
                let mode = (round + offset) % 4;
                samples[mode].push(batch(mode, counts[mode])?.as_secs_f64() * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"width\":{},\"height\":{},\"bytes\":{:?},\"iterations\":{:?},\"samples_us\":{:?}}}",
            w, h, blobs.iter().map(Vec::len).collect::<Vec<_>>(), counts, samples);
    }
    Ok(())
}
'''
    (folder / "src/main.rs").write_text(driver, encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path", str(folder / "Cargo.toml")],
                   check=True)
    binary = folder / "target/release" / ("raipixel-match-depth.exe" if sys.platform == "win32" else "raipixel-match-depth")
    lines = subprocess.check_output([str(binary), *sys.argv[1:]], text=True).splitlines()
    if len(lines) != len(sys.argv[1:]):
        raise ValueError("missing benchmark cases")
    cases = []
    for path, line in zip(sys.argv[1:], lines):
        case = json.loads(line)
        medians = [sorted(s)[len(s)//2] for s in case["samples_us"]]
        case.update(input=path, median_us=medians,
                    size_over_current=[v/case["bytes"][0] for v in case["bytes"]],
                    time_over_current=[v/medians[0] for v in medians],
                    input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(case)
    print(json.dumps({
        "experiment":"safe_fixed_huffman_match_selection_scratch",
        "modes":["current", "lazy_one_step", "two_entries_per_hash", "both"],
        "compiler":subprocess.check_output(["rustc","--version","--verbose"],text=True),
        "platform":platform.platform(), "toolchain":os.environ.get("RUSTUP_TOOLCHAIN", ""),
        "source_sha256":hashlib.sha256(SRC.encode()).hexdigest(),
        "candidates_sha256":fingerprints, "rounds":9,"minimum_batch_ms":25,
        "all_png_outputs_verified":True, "cases":cases
    }))
