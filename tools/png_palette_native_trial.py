"""Native PNG indexed-color feasibility with our OWN compact DEFLATE writer.

A temporary copy of src/png.rs adds trial_encode_palette using std HashMap,
PNG PLTE/tRNS and the already shipped fixed-Huffman block_compact compressor.
Returns false (writing nothing) if RGBA colors exceed 256. Benchmarks full PNG
encoding, reports exact size, and tests deterministic output, transparent-RGB
preservation, CRC rejection, and budget failure. Production is unchanged.
No png crate source, new runtime dependency or unsafe Rust.

Run: python tools/png_palette_native_trial.py INPUT.png [INPUT.png ...]
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
CANDIDATE = r'''
/// Experiment ONLY: exact RGBA8 to indexed PNG8, with the accepted DEFLATE
/// fixed-code compact matcher, preserving fully transparent RGB colors.
pub fn trial_encode_palette(
    image: &Image, output: &mut impl std::io::Write, work: &mut Work
) -> Result<bool, Error> {
    use std::collections::HashMap;
    let pixels = image.pixels();
    work.charge(pixels.len() as u64)?;
    let mut indices = HashMap::<[u8;4], u8>::new();
    let mut colors = Vec::<[u8;4]>::new();
    for &sample in pixels {
        if let std::collections::hash_map::Entry::Vacant(entry) = indices.entry(sample) {
            if colors.len() == 256 {
                return Ok(false);
            }
            let next = colors.len() as u8;
            entry.insert(next);
            colors.push(sample);
        }
    }
    let row_bytes = image.width as usize;
    let stride = row_bytes + 1;
    let raw_size = stride * image.height as usize;
    let blocks = raw_size.div_ceil(crate::deflate::BLOCK);
    work.charge((raw_size * 16 + blocks * (8192 + 64)) as u64)?;
    let capacity = raw_size.min(crate::deflate::BLOCK);
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity).map_err(|_| crate::Error::Allocation)?;
    let mut encoder = crate::deflate::Encoder::new(capacity)?;
    let mut palette = Vec::new();
    palette.try_reserve_exact(colors.len() * 3).map_err(|_| crate::Error::Allocation)?;
    let mut alpha = Vec::new();
    alpha.try_reserve_exact(colors.len()).map_err(|_| crate::Error::Allocation)?;
    for color in &colors {
        palette.extend_from_slice(&color[..3]);
        alpha.push(color[3]);
    }
    let mut ihdr = [0u8; 13];
    ihdr[..4].copy_from_slice(&image.width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&image.height.to_be_bytes());
    ihdr[8] = 8;
    ihdr[9] = 3;
    output.write_all(SIGNATURE)?;
    write_chunk(output, b"IHDR", &ihdr)?;
    write_chunk(output, b"PLTE", &palette)?;
    if alpha.iter().any(|&a| a != 255) {
        write_chunk(output, b"tRNS", &alpha)?;
    }
    write_chunk(output, b"IDAT", &[0x78, 0x01])?;
    let mut offset = 0;
    let mut adler = Adler::new();
    while offset < raw_size {
        let len = (raw_size - offset).min(capacity);
        raw.resize(len, 0);
        for (i, byte) in raw.iter_mut().enumerate() {
            let position = offset + i;
            let row = position / stride;
            let col = position % stride;
            if col != 0 {
                let sample = pixels[row * row_bytes + col - 1];
                *byte = indices[&sample];
            }
        }
        adler.update(&raw);
        let compressed = encoder.block_compact(&raw, offset + len == raw_size);
        write_chunk(output, b"IDAT", compressed)?;
        offset += len;
    }
    write_chunk(output, b"IDAT", &adler.value().to_be_bytes())?;
    write_chunk(output, b"IEND", &[])?;
    Ok(true)
}
'''
with tempfile.TemporaryDirectory(prefix="raipixel-native-palette-") as directory:
    folder = Path(directory)
    scratch = folder / "trial"
    shutil.copytree(ROOT / "src", scratch / "src")
    edited = SOURCE + "\n" + CANDIDATE
    (scratch / "src/png.rs").write_text(edited, encoding="utf-8")
    (scratch / "Cargo.toml").write_text(
        '[package]\nname="raipixel_native_palette"\nversion="0.0.0"\nedition="2024"\n'
        '[lib]\npath="src/lib.rs"\n', encoding="utf-8")
    (folder / "src").mkdir()
    (folder / "Cargo.toml").write_text(
        '[package]\nname="raipixel-palette-screen"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n'
        'candidate={package="raipixel_native_palette",path=' + json.dumps(str(scratch)) + '}\n',
        encoding="utf-8")
    driver = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Duration, Instant}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args().skip(1) {
        let input = std::fs::read(&path)?;
        let original = raipixel::png::decode(&input, raipixel::Limits::default(), &mut raipixel::Work::default())?.image;
        let (w,h) = (original.size()[0], original.size()[1]);
        let trial_image = candidate::Image::from_pixels(w,h,original.pixels(),
            candidate::Limits::default(),&mut candidate::Work::default())?;
        let mut accepted = Vec::new();
        raipixel::png::encode_compact(&original, &mut accepted, &mut raipixel::Work::default())?;
        let mut palette = Vec::new();
        let possible = candidate::png::trial_encode_palette(
            &trial_image, &mut palette, &mut candidate::Work::default())?;
        if possible {
            let decoded = raipixel::png::decode(&palette, raipixel::Limits::default(), &mut raipixel::Work::default())?;
            assert_eq!(decoded.image, original);
            assert_eq!(decoded.image.pixels().as_flattened(), original.pixels().as_flattened());
            let mut again = Vec::new();
            assert!(candidate::png::trial_encode_palette(
                &trial_image,&mut again,&mut candidate::Work::default())?);
            assert_eq!(palette, again, "palette output must be deterministic");
            let mut invalid = palette.clone();
            invalid[29] ^= 1;
            assert!(matches!(raipixel::png::decode(&invalid, raipixel::Limits::default(),
                &mut raipixel::Work::default()), Err(raipixel::png::Error::Crc)));
            let mut rejected = Vec::new();
            assert!(candidate::png::trial_encode_palette(
                &trial_image, &mut rejected, &mut candidate::Work::new(0)?) .is_err());
            assert!(rejected.is_empty());
        } else {
            assert!(palette.is_empty(), "unsupported palette must not publish partial PNG");
        }
        let bench = |mode: usize, n: usize| -> Result<Duration, Box<dyn std::error::Error>> {
            let start = Instant::now();
            for _ in 0..n {
                let mut out = Vec::new();
                if mode == 0 {
                    raipixel::png::encode_compact(black_box(&original), &mut out, &mut raipixel::Work::default())?;
                } else {
                    assert!(candidate::png::trial_encode_palette(black_box(&trial_image),
                        &mut out, &mut candidate::Work::default())?);
                }
                black_box(out);
            }
            Ok(start.elapsed())
        };
        if !possible {
            println!("{{\"supported\":false,\"width\":{},\"height\":{},\"compact_bytes\":{}}}",w,h,accepted.len());
            continue;
        }
        let mut counts = [1usize; 2];
        for (mode,count) in counts.iter_mut().enumerate() {
            while bench(mode,*count)? < Duration::from_millis(25) {
                *count = count.checked_mul(2).ok_or("benchmark overflow")?;
            }
        }
        let mut samples = [Vec::new(),Vec::new()];
        for round in 0..9 {
            for mode in [round%2,(round+1)%2] {
                samples[mode].push(bench(mode,counts[mode])?.as_secs_f64()*1e6/counts[mode] as f64);
            }
        }
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"compact_bytes\":{},\"palette_bytes\":{},\"samples_us\":{:?}}}",
            w,h,accepted.len(),palette.len(),samples);
    }
    Ok(())
}
'''
    (folder / "src/main.rs").write_text(driver, encoding="utf-8")
    subprocess.run(["cargo","build","--release","--offline","--manifest-path",str(folder / "Cargo.toml")],check=True)
    executable = folder / "target/release" / (
        "raipixel-palette-screen.exe" if sys.platform=="win32" else "raipixel-palette-screen")
    lines = subprocess.check_output([str(executable),*sys.argv[1:]],text=True).splitlines()
    assert len(lines) == len(sys.argv[1:])
    cases = []
    for path,line in zip(sys.argv[1:],lines):
        row = json.loads(line)
        row["input"] = path
        row["input_sha256"] = hashlib.sha256(Path(path).read_bytes()).hexdigest()
        if row["supported"]:
            med = [sorted(s)[len(s)//2] for s in row["samples_us"]]
            row["median_us"] = med
            row["time_over_compact"] = med[1]/med[0]
            row["size_over_compact"] = row["palette_bytes"]/row["compact_bytes"]
        cases.append(row)
    print(json.dumps({"experiment":"own_fixed_huffman_indexed_png8_size_and_speed",
                      "reference":"production encode_compact, same matching algorithm",
                      "candidate":"temporary safe-Rust indexed writer, exact RGBA tRNS, no new dependencies",
                      "compiler":subprocess.check_output(["rustc","--version","--verbose"],text=True),
                      "platform":platform.platform(),"rounds":9,"minimum_batch_ms":25,
                      "source_sha256":hashlib.sha256(SOURCE.encode()).hexdigest(),
                      "candidate_sha256":hashlib.sha256(CANDIDATE.encode()).hexdigest(),
                      "cases":cases}))
