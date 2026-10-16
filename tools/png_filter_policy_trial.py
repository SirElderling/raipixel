"""Screen PNG Sub/Up row-filter policies in scratch copies of OUR writer.

The accepted writer, all-Sub, and two sampled Sub/Up policies are compiled
together under the same Rust 1.99.1 release profile. All tests verify every
RGBA byte, filtered-stream correctness via the accepted decoder, deterministic
writer bytes, and the same remaining work budget. No third-party source/code,
unsafe Rust, dependencies or production changes.

Run after generating png-stage-inputs and png-original-inputs:
    python tools/png_filter_policy_trial.py INPUT.png [INPUT.png ...]
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
ORIGINAL = (ROOT / "src/png.rs").read_text(encoding="utf-8")

def once(text, old, new):
    if text.count(old) != 1:
        raise ValueError("PNG writer anchor missing or ambiguous: " + repr(old[:70]))
    return text.replace(old, new)

def variant(policy):
    # Modify ONLY the compressed writer; preserve stored encoder and decoder.
    boundary = ORIGINAL.index("pub fn encode_compressed(")
    original_prefix, text = ORIGINAL[:boundary], ORIGINAL[boundary:]
    if policy == "all_sub":
        text = once(text, "raw[filled] = if row == 0 { 1 } else { 2 };",
                    "raw[filled] = 1;")
        text = once(text, "if row == 0 {\n                    for (i, value)",
                    "if true {\n                    for (i, value)")
    else:
        sample = 16 if policy == "sample_16" else 64
        helper = f"""
// Diagnostic: choose between valid PNG Sub/Up filters by a bounded sample
// of signed byte residuals across four evenly spaced row windows.
fn trial_choose_filter(row: usize, stride: usize, rgba: &[u8]) -> u8 {{
    if row == 0 {{ return 1; }}
    let base = row * stride;
    let window = stride.min({sample});
    let mut score_sub = 0u32;
    let mut score_up = 0u32;
    for segment in 0..4usize {{
        let begin = (stride - window) * segment / 3;
        for i in begin..begin + window {{
            let value = rgba[base + i];
            let left = if i >= 4 {{ rgba[base + i - 4] }} else {{ 0 }};
            let above = rgba[base + i - stride];
            score_sub += u32::from((value.wrapping_sub(left) as i8 as i16).unsigned_abs());
            score_up += u32::from((value.wrapping_sub(above) as i8 as i16).unsigned_abs());
        }}
    }}
    if score_sub < score_up {{ 1 }} else {{ 2 }}
}}
"""
        original_prefix += helper
        text = once(text, "    let mut adler = Adler::new();\n    while offset < raw_size {",
                    "    let mut adler = Adler::new();\n    let mut row_filter = 1u8;\n    while offset < raw_size {")
        text = once(text, "raw[filled] = if row == 0 { 1 } else { 2 };",
                    "row_filter = trial_choose_filter(row, row_bytes, rgba);\n                raw[filled] = row_filter;")
        text = once(text, "if row == 0 {\n                    for (i, value)",
                    "if row_filter == 1 {\n                    for (i, value)")
    text = once(text, "if x < 4 { 0 } else { rgba[x - 4] }",
                "if x < row * row_bytes + 4 { 0 } else { rgba[x - 4] }")
    # Fix all-Sub output for first and following rows; replacing conditional
    # with true is deliberate scratch-only policy, not production style.
    return original_prefix + text

with tempfile.TemporaryDirectory(prefix="raipixel-filter-policies-") as directory:
    scratch = Path(directory)
    candidates = [("sub", "raipixel_sub", "all_sub"),
                  ("sample16", "raipixel_sample16", "sample_16"),
                  ("sample64", "raipixel_sample64", "sample_64")]
    changed = {}
    for alias, package, policy in candidates:
        target = scratch / alias
        shutil.copytree(ROOT / "src", target / "src")
        edited = variant(policy)
        (target / "src/png.rs").write_text(edited, encoding="utf-8")
        (target / "Cargo.toml").write_text(
            f'[package]\nname="{package}"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n', encoding="utf-8")
        changed[alias] = hashlib.sha256(edited.encode()).hexdigest()
    manifest = '[package]\nname="raipixel-filter-screen"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
    manifest += 'raipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    for alias, package, _ in candidates:
        manifest += alias + '={package=' + json.dumps(package) + ',path=' + json.dumps(str(scratch / alias)) + '}\n'
    (scratch / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (scratch / "src").mkdir()
    driver = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Duration, Instant}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args().skip(1) {
        let input = std::fs::read(&path)?;
        let original = raipixel::png::decode(&input, raipixel::Limits::default(), &mut raipixel::Work::default())?.image;
        let (w, h) = (original.size()[0], original.size()[1]);
        let pixels = original.pixels();
        let a = sub::Image::from_pixels(w, h, pixels, sub::Limits::default(), &mut sub::Work::default())?;
        let b = sample16::Image::from_pixels(w, h, pixels, sample16::Limits::default(), &mut sample16::Work::default())?;
        let c = sample64::Image::from_pixels(w, h, pixels, sample64::Limits::default(), &mut sample64::Work::default())?;
        let mut blobs = Vec::new();
        let mut work_left = Vec::new();
        for mode in 0..4 {
            let mut out = Vec::new();
            let remaining = match mode {
                0 => {
                    let mut work = raipixel::Work::default();
                    raipixel::png::encode_compressed(&original, &mut out, &mut work)?;
                    work.remaining()
                }
                1 => {
                    let mut work = sub::Work::default();
                    sub::png::encode_compressed(&a, &mut out, &mut work)?;
                    work.remaining()
                }
                2 => {
                    let mut work = sample16::Work::default();
                    sample16::png::encode_compressed(&b, &mut out, &mut work)?;
                    work.remaining()
                }
                3 => {
                    let mut work = sample64::Work::default();
                    sample64::png::encode_compressed(&c, &mut out, &mut work)?;
                    work.remaining()
                }
                _ => unreachable!()
            };
            let decoded = raipixel::png::decode(&out, raipixel::Limits::default(), &mut raipixel::Work::default())?;
            assert_eq!(decoded.image.pixels(), pixels);
            work_left.push(remaining);
            blobs.push(out);
        }
        assert!(work_left.iter().all(|&x| x == work_left[0]), "work charge changed");
        let batch = |mode: usize, count: usize| -> Result<Duration, Box<dyn std::error::Error>> {
            let start = Instant::now();
            for _ in 0..count {
                let mut out = Vec::new();
                match mode {
                    0 => raipixel::png::encode_compressed(black_box(&original), &mut out, &mut raipixel::Work::default())?,
                    1 => sub::png::encode_compressed(black_box(&a), &mut out, &mut sub::Work::default())?,
                    2 => sample16::png::encode_compressed(black_box(&b), &mut out, &mut sample16::Work::default())?,
                    3 => sample64::png::encode_compressed(black_box(&c), &mut out, &mut sample64::Work::default())?,
                    _ => unreachable!()
                }
                black_box(out);
            }
            Ok(start.elapsed())
        };
        let mut counts = [1usize; 4];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count)? < Duration::from_millis(25) {
                *count = count.checked_mul(2).ok_or("benchmark iteration overflow")?;
            }
        }
        let mut samples: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
        for round in 0..9 {
            for offset in 0..4 {
                let mode = (round + offset) % 4;
                samples[mode].push(batch(mode, counts[mode])?.as_secs_f64() * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"width\":{},\"height\":{},\"bytes\":{:?},\"work_left\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",
            w, h, blobs.iter().map(Vec::len).collect::<Vec<_>>(), work_left[0], counts, samples);
    }
    Ok(())
}
'''
    (scratch / "src/main.rs").write_text(driver, encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path",
                    str(scratch / "Cargo.toml")], check=True)
    executable = scratch / "target/release" / (
        "raipixel-filter-screen.exe" if sys.platform == "win32" else "raipixel-filter-screen")
    lines = subprocess.check_output([str(executable), *sys.argv[1:]], text=True).splitlines()
    assert len(lines) == len(sys.argv[1:]), "incomplete native test output"
    cases = []
    for path, line in zip(sys.argv[1:], lines):
        obj = json.loads(line)
        median = [sorted(a)[len(a)//2] for a in obj["samples_us"]]
        obj.update(input=path, median_us=median,
                   size_over_current=[v/obj["bytes"][0] for v in obj["bytes"]],
                   time_over_current=[v/median[0] for v in median],
                   input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(obj)
    print(json.dumps(dict(experiment="png_sub_up_filter_policy_scratch",
                          modes=["current_sub_first_up_rest", "all_sub", "sampled_sub_up_16", "sampled_sub_up_64"],
                          compiler=subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
                          platform=platform.platform(), rustup_toolchain=os.environ.get("RUSTUP_TOOLCHAIN", ""),
                          source_sha256=hashlib.sha256(ORIGINAL.encode()).hexdigest(),
                          candidate_sha256=changed, samples_per_mode=9,
                          all_outputs_decoded_and_work_equal=True, cases=cases)))
