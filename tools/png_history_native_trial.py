"""Scratch-only cross-block 32 KiB LZ77 history and bounded hash-chain depth.

This is independently authored safe Rust. It leaves the accepted fast,
compact and indexed writers untouched. All candidates use the real indexed
PNG writer with a scratch encoder, compare whole-output byte lengths/timing,
and validate decoded RGBA, CRC rejection, work accounting and determinism.
Temporary benchmark candidate matching state is NOT included in the product
work-budget charge, so candidates cannot be promoted unchanged.

Run: python tools/png_history_native_trial.py INPUT.png ...
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
ENCODER = (ROOT / "src/deflate/encode.rs").read_text(encoding="utf-8")
PNG = (ROOT / "src/png.rs").read_text(encoding="utf-8")

HISTORY = r'''
impl Encoder {
    // Scratch only: retain 32 KiB of uncompressed input history across
    // consecutive DEFLATE blocks; encode the current block only.
    pub(crate) fn block_history(&mut self, data: &[u8], last: bool) -> &[u8] {
        const DEPTH: usize = TRIAL_DEPTH;
        debug_assert!(data.len() <= BLOCK);
        let mut joined = Vec::with_capacity(self.history.len() + data.len());
        joined.extend_from_slice(&self.history);
        joined.extend_from_slice(data);
        let start = self.history.len();
        let mut heads = vec![u32::MAX; HASH_SIZE];
        let mut links = vec![u32::MAX; joined.len()];
        for pos in 0..start {
            if pos + 3 > joined.len() { break; }
            let (slot, _) = hash(&joined, pos);
            links[pos] = heads[slot];
            heads[slot] = pos as u32;
        }
        let saved = (self.bits, self.count);
        self.output.clear();
        self.put(u32::from(last) | 2, 3);
        let mut pos = start;
        while pos + 3 <= joined.len() {
            let (slot, _) = hash(&joined, pos);
            let maximum = (joined.len() - pos).min(258);
            let mut best = 0usize;
            let mut best_pos = 0usize;
            let mut prior = heads[slot];
            for _ in 0..DEPTH {
                if prior == u32::MAX { break; }
                let candidate = prior as usize;
                if pos - candidate > 32_768 { break; }
                if joined[candidate..candidate + 3] == joined[pos..pos + 3] {
                    let length = 3 + matching_tail(
                        &joined[candidate + 3..candidate + maximum],
                        &joined[pos + 3..pos + maximum],
                    );
                    if length > best {
                        best = length;
                        best_pos = candidate;
                        if length == maximum { break; }
                    }
                }
                prior = links[candidate];
            }
            links[pos] = heads[slot];
            heads[slot] = pos as u32;
            if pos > 0 && joined[pos] == joined[pos - 1] {
                let length = repeated_length(&joined, pos - 1, pos, maximum);
                if length >= best {
                    best = length;
                    best_pos = pos - 1;
                }
            }
            if best >= 3 {
                self.reference(best, pos - best_pos);
                let end = pos + best;
                pos += 1;
                while pos < end {
                    if pos + 3 <= joined.len() {
                        let (slot, _) = hash(&joined, pos);
                        links[pos] = heads[slot];
                        heads[slot] = pos as u32;
                    }
                    pos += 1;
                }
            } else {
                self.symbol(u32::from(joined[pos]));
                pos += 1;
            }
        }
        for &byte in &joined[pos..] {
            self.symbol(u32::from(byte));
        }
        self.symbol(256);
        if last { self.align(); }
        self.flush_bytes();
        if self.output.len() > data.len() + 5 {
            (self.bits, self.count) = saved;
            self.store(data, last);
        }
        // The decompressor sees stored and Huffman-coded bytes equally:
        // either form contributes to the same sliding history.
        let keep = joined.len().min(32_768);
        self.history.clear();
        self.history.extend_from_slice(&joined[joined.len() - keep..]);
        &self.output
    }
}
'''

def replace_once(text, old, new):
    count = text.count(old)
    if count != 1:
        raise ValueError(f"unstable source anchor {old[:80]!r}, found {count}")
    return text.replace(old, new)

def candidate(depth, dynamic=False):
    encoder = replace_once(
        ENCODER, "    dictionary: Vec<u32>,\n", "    dictionary: Vec<u32>,\n    history: Vec<u8>,\n"
    )
    encoder = replace_once(
        encoder, "            dictionary,\n            output,", "            dictionary,\n            history: Vec::new(),\n            output,"
    )
    encoder += "\n" + HISTORY.replace("TRIAL_DEPTH", str(depth))
    if dynamic:
        # Import only our own vetted scratch RFC 1951 Huffman routines,
        # never any implementation code from the reference png crate.
        trial_source = (ROOT / "tools/png_dynamic_indexed_guard_trial.py").read_text(encoding="utf-8")
        helper = trial_source.split("HELPER=r'''", 1)[1].split("\n'''", 1)[0]
        start = encoder.index("    pub(crate) fn block_history(")
        unchanged, method = encoder[:start], encoder[start:]
        method = replace_once(method,
            "        let saved = (self.bits, self.count);\n        self.output.clear();\n        self.put(u32::from(last) | 2, 3);",
            "        let saved = (self.bits, self.count);\n        let mut tokens = Vec::<u16>::new();\n        self.output.clear();")
        method = replace_once(method,
            "                self.reference(best, pos - best_pos);",
            "                tokens.push(256 + (best - 3) as u16);\n                tokens.push((pos - best_pos) as u16);")
        method = replace_once(method,
            "                self.symbol(u32::from(joined[pos]));",
            "                tokens.push(u16::from(joined[pos]));")
        method = replace_once(method,
            "        for &byte in &joined[pos..] {\n            self.symbol(u32::from(byte));\n        }",
            "        for &byte in &joined[pos..] { tokens.push(u16::from(byte)); }")
        method = replace_once(method,
            """        self.symbol(256);
        if last { self.align(); }
        self.flush_bytes();
        if self.output.len() > data.len() + 5 {
            (self.bits, self.count) = saved;
            self.store(data, last);
        }""",
            """        let fixed_bits = trial_fixed_bits(&tokens);
        let fixed_bytes = ((saved.1 + fixed_bits + if last { 7 } else { 0 }) / 8) as usize;
        // Skip all tree work for tiny outputs and fall back if no savings.
        let dynamic_ok = data.len() >= 4096 && self.trial_emit_dynamic(&tokens, last);
        if !dynamic_ok || self.output.len() >= fixed_bytes.min(data.len() + 5) {
            self.output.clear();
            (self.bits, self.count) = saved;
            if fixed_bytes > data.len() + 5 {
                self.store(data, last);
            } else {
                self.put(u32::from(last) | 2, 3);
                for token in trial_unpack(&tokens) {
                    match token {
                        TrialToken::Literal(value) => self.symbol(u32::from(value)),
                        TrialToken::Reference(length, distance) => self.reference(length, distance),
                    }
                }
                self.symbol(256);
                if last { self.align(); }
                self.flush_bytes();
            }
        }""")
        encoder = unchanged + method + "\n" + helper
    png = replace_once(
        PNG,
        "let compressed = encoder.block_compact(&raw, offset + len == raw_size);",
        "let compressed = encoder.block_history(&raw, offset + len == raw_size);"
    )
    return encoder, png

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
        let a = history1::Image::from_pixels(width, height, base.pixels(),
                        history1::Limits::default(), &mut history1::Work::default())?;
        let b = history8::Image::from_pixels(width, height, base.pixels(),
                        history8::Limits::default(), &mut history8::Work::default())?;
        let c = history16::Image::from_pixels(width, height, base.pixels(),
                        history16::Limits::default(), &mut history16::Work::default())?;
        let d = history16dyn::Image::from_pixels(width, height, base.pixels(),
                        history16dyn::Limits::default(), &mut history16dyn::Work::default())?;
        let mut outputs: [Vec<u8>; 5] = std::array::from_fn(|_| Vec::new());
        let mut budgets = [0u64; 5];
        let mut supported = [false; 5];
        for mode in 0..5 {
            let remaining = match mode {
                0 => {
                    let mut work = raipixel::Work::default();
                    supported[mode] = raipixel::png::encode_indexed(&base, &mut outputs[mode], &mut work)?;
                    work.remaining()
                }
                1 => {
                    let mut work = history1::Work::default();
                    supported[mode] = history1::png::encode_indexed(&a, &mut outputs[mode], &mut work)?;
                    work.remaining()
                }
                2 => {
                    let mut work = history8::Work::default();
                    supported[mode] = history8::png::encode_indexed(&b, &mut outputs[mode], &mut work)?;
                    work.remaining()
                }
                3 => {
                    let mut work = history16::Work::default();
                    supported[mode] = history16::png::encode_indexed(&c, &mut outputs[mode], &mut work)?;
                    work.remaining()
                }
                _ => {
                    let mut work = history16dyn::Work::default();
                    supported[mode] = history16dyn::png::encode_indexed(&d, &mut outputs[mode], &mut work)?;
                    work.remaining()
                }
            };
            budgets[mode] = remaining;
        }
        assert!(supported.iter().all(|&v| v == supported[0]), "different palette support");
        assert!(budgets.iter().all(|&v| v == budgets[0]), "changed work charge");
        if !supported[0] {
            assert!(outputs.iter().all(Vec::is_empty), "unsupported PNG wrote bytes");
            println!("{{\"supported\":false,\"width\":{},\"height\":{}}}", width, height);
            continue;
        }
        for mode in 0..outputs.len() {
            assert_eq!(raipixel::png::decode(&outputs[mode], raipixel::Limits::default(),
                           &mut raipixel::Work::default())?.image, base,
                       "RGBA difference in mode {}", mode);
            let mut corrupt = outputs[mode].clone();
            corrupt[29] ^= 1;
            assert!(matches!(raipixel::png::decode(&corrupt, raipixel::Limits::default(),
                            &mut raipixel::Work::default()), Err(raipixel::png::Error::Crc)));
        }
        let once = |mode: usize| -> Result<Vec<u8>, Failure> {
            let mut out = Vec::new();
            match mode {
                0 => { assert!(raipixel::png::encode_indexed(black_box(&base), &mut out, &mut raipixel::Work::default())?); }
                1 => { assert!(history1::png::encode_indexed(black_box(&a), &mut out, &mut history1::Work::default())?); }
                2 => { assert!(history8::png::encode_indexed(black_box(&b), &mut out, &mut history8::Work::default())?); }
                3 => { assert!(history16::png::encode_indexed(black_box(&c), &mut out, &mut history16::Work::default())?); }
                _ => { assert!(history16dyn::png::encode_indexed(black_box(&d), &mut out, &mut history16dyn::Work::default())?); }
            }
            Ok(out)
        };
        for mode in 0..outputs.len() {
            assert_eq!(once(mode)?, outputs[mode], "nondeterminism in mode {}", mode);
        }
        let batch = |mode: usize, count: usize| -> Result<Duration, Failure> {
            let start = Instant::now();
            for _ in 0..count { black_box(once(mode)?); }
            Ok(start.elapsed())
        };
        let mut counts = [1usize; 5];
        for mode in 0..5 {
            while batch(mode, counts[mode])? < Duration::from_millis(25) {
                counts[mode] = counts[mode].checked_mul(2).ok_or("iteration overflow")?;
            }
        }
        let mut samples: [Vec<f64>; 5] = std::array::from_fn(|_| Vec::new());
        for round in 0..9 {
            for item in 0..5 {
                let mode = (item + round) % 5;
                samples[mode].push(batch(mode, counts[mode])?.as_secs_f64() *
                                   1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"bytes\":{:?},\"samples_us\":{:?}}}",
                 width, height, outputs.map(|v| v.len()), samples);
    }
    Ok(())
}
'''

with tempfile.TemporaryDirectory(prefix="raipixel-cross-history-") as temp:
    folder = Path(temp)
    deps = ['raipixel={path=' + json.dumps(str(ROOT)) + '}']
    candidates = {}
    for name, depth, dynamic in (("history1", 1, False), ("history8", 8, False),
                                 ("history16", 16, False), ("history16dyn", 16, True)):
        scratch = folder / name
        shutil.copytree(ROOT / "src", scratch / "src")
        modified_deflate, modified_png = candidate(depth, dynamic)
        (scratch / "src/deflate/encode.rs").write_text(modified_deflate, encoding="utf-8")
        (scratch / "src/png.rs").write_text(modified_png, encoding="utf-8")
        (scratch / "Cargo.toml").write_text(
            f'[package]\nname="raipixel_{name}"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n', encoding="utf-8")
        deps.append(f'{name}={{package="raipixel_{name}",path='
                    + json.dumps(str(scratch)) + '}')
        candidates[name] = hashlib.sha256(
            (modified_deflate + modified_png).encode()).hexdigest()
    (folder / "src").mkdir()
    (folder / "Cargo.toml").write_text(
        '[package]\nname="raipixel-history-screen"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\n' + '\n'.join(deps) + '\n', encoding="utf-8")
    (folder / "src/main.rs").write_text(DRIVER, encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path",
                    str(folder / "Cargo.toml")], check=True)
    exe = folder / "target/release" / (
        "raipixel-history-screen.exe" if sys.platform == "win32" else "raipixel-history-screen")
    lines = subprocess.check_output([str(exe), *sys.argv[1:]], text=True).splitlines()
    if len(lines) != len(sys.argv[1:]):
        raise ValueError("missing history benchmark cases")
    cases = []
    for path, line in zip(sys.argv[1:], lines):
        case = json.loads(line)
        if case["supported"]:
            medians = [sorted(sample)[len(sample)//2] for sample in case["samples_us"]]
            case.update(median_us=medians, size_over_baseline=[
                            v / case["bytes"][0] for v in case["bytes"]],
                        time_over_baseline=[v/medians[0] for v in medians])
        case.update(input=path, input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(case)
    print(json.dumps(dict(experiment="cross_block_32k_history_1_8_16_and_dynamic_candidates",
                          platform=platform.platform(),
                          compiler=subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
                          encoder_sha256=hashlib.sha256(ENCODER.encode()).hexdigest(),
                          png_sha256=hashlib.sha256(PNG.encode()).hexdigest(),
                          candidates_sha256=candidates,
                          driver_sha256=hashlib.sha256(DRIVER.encode()).hexdigest(),
                          rounds=9, minimum_batch_ms=25,
                          exact_rgba_crc_work_determinism_verified=True,
                          production_changed=False, cases=cases)))
