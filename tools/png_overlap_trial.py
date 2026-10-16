"""Screen safe DEFLATE short-overlap copying; production codec unchanged.

Two individual 2-byte/4-byte repeated-pattern fill candidates and their
combination are constructed from OUR own DEFLATE decoder in temporary crates.
Tests valid image equality/flags/work, independent malformed-CRC rejection,
and paired complete-PNG decode across real corpus images. No third-party
implementation source inspected, dependencies, intrinsics, or unsafe Rust.

Usage after exporting PNG corpus:
    python tools/png_overlap_trial.py INPUT.png [INPUT.png ...]
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
SRC = (ROOT / "src/deflate.rs").read_text(encoding="utf-8")
ANCHOR = """                            } else {
                                let start = out.len() - distance;
                                let mut remaining = len;"""
assert SRC.count(ANCHOR) == 1, "The existing DEFLATE copy path changed"

def make_variant(two, four):
    prefix = ""
    if two:
        prefix += """                            } else if distance == 2 {
                                // The entire match is an alternating repetition
                                // of the two history bytes. Read before resize.
                                let start = out.len();
                                let pair = [out[start - 2], out[start - 1]];
                                out.resize(start + len, 0);
                                for (i, value) in out[start..].iter_mut().enumerate() {
                                    *value = pair[i & 1];
                                }
"""
    if four:
        prefix += """                            } else if distance == 4 {
                                // Same proof for a four-byte periodic history.
                                let start = out.len();
                                let pattern = [
                                    out[start - 4], out[start - 3],
                                    out[start - 2], out[start - 1],
                                ];
                                out.resize(start + len, 0);
                                for (i, value) in out[start..].iter_mut().enumerate() {
                                    *value = pattern[i & 3];
                                }
"""
    return SRC.replace(ANCHOR, prefix + ANCHOR) if prefix else SRC

with tempfile.TemporaryDirectory(prefix="raipixel-overlap-") as path:
    root = Path(path)
    packages = [
        ("distance2", "raipixel_distance2", True, False),
        ("distance4", "raipixel_distance4", False, True),
        ("both", "raipixel_distance24", True, True)
    ]
    hashes = {}
    for alias, package, two, four in packages:
        directory = root / alias
        shutil.copytree(ROOT / "src", directory / "src")
        candidate = make_variant(two, four)
        (directory / "src/deflate.rs").write_text(candidate, encoding="utf-8")
        (directory / "Cargo.toml").write_text(
            f'[package]\nname="{package}"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n', encoding="utf-8")
        hashes[alias] = hashlib.sha256(candidate.encode()).hexdigest()

    manifest = '[package]\nname="raipixel-overlap-screen"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
    manifest += 'raipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    for alias, package, _, _ in packages:
        manifest += alias + '={package=' + json.dumps(package) + ',path=' + json.dumps(str(root / alias)) + '}\n'
    (root / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (root / "src").mkdir()
    driver = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Duration, Instant}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args().skip(1) {
        let input = std::fs::read(&path)?;
        let mut a_work = raipixel::Work::default();
        let a = raipixel::png::decode(&input, raipixel::Limits::default(), &mut a_work)?;
        let mut b_work = distance2::Work::default();
        let b = distance2::png::decode(&input, distance2::Limits::default(), &mut b_work)?;
        let mut c_work = distance4::Work::default();
        let c = distance4::png::decode(&input, distance4::Limits::default(), &mut c_work)?;
        let mut d_work = both::Work::default();
        let d = both::png::decode(&input, both::Limits::default(), &mut d_work)?;
        for (samples, rem, flags) in [
            (b.image.pixels(), b_work.remaining(), (b.skipped_ancillary, b.uninterpreted_color, b.trailing_idat_bytes)),
            (c.image.pixels(), c_work.remaining(), (c.skipped_ancillary, c.uninterpreted_color, c.trailing_idat_bytes)),
            (d.image.pixels(), d_work.remaining(), (d.skipped_ancillary, d.uninterpreted_color, d.trailing_idat_bytes))
        ] {
            assert_eq!(samples, a.image.pixels());
            assert_eq!(rem, a_work.remaining());
            assert_eq!(flags, (a.skipped_ancillary, a.uninterpreted_color, a.trailing_idat_bytes));
        }
        // Unlike the earlier deliberately checksum-free *diagnostic*, every
        // version of this experiment validates all PNG chunk checksums.
        let mut corrupted = input.clone();
        corrupted[29] ^= 1;
        assert!(matches!(raipixel::png::decode(&corrupted, raipixel::Limits::default(),
            &mut raipixel::Work::default()), Err(raipixel::png::Error::Crc)));
        assert!(matches!(distance2::png::decode(&corrupted, distance2::Limits::default(),
            &mut distance2::Work::default()), Err(distance2::png::Error::Crc)));
        assert!(matches!(distance4::png::decode(&corrupted, distance4::Limits::default(),
            &mut distance4::Work::default()), Err(distance4::png::Error::Crc)));
        assert!(matches!(both::png::decode(&corrupted, both::Limits::default(),
            &mut both::Work::default()), Err(both::png::Error::Crc)));
        let batch = |mode: usize, count: usize| -> Result<Duration, Box<dyn std::error::Error>> {
            let start = Instant::now();
            for _ in 0..count {
                match mode {
                    0 => { black_box(raipixel::png::decode(black_box(&input),
                            raipixel::Limits::default(), &mut raipixel::Work::default())?); }
                    1 => { black_box(distance2::png::decode(black_box(&input),
                            distance2::Limits::default(), &mut distance2::Work::default())?); }
                    2 => { black_box(distance4::png::decode(black_box(&input),
                            distance4::Limits::default(), &mut distance4::Work::default())?); }
                    3 => { black_box(both::png::decode(black_box(&input),
                            both::Limits::default(), &mut both::Work::default())?); }
                    _ => unreachable!()
                };
            }
            Ok(start.elapsed())
        };
        let mut counts = [1usize; 4];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count)? < Duration::from_millis(25) {
                *count = count.checked_mul(2).ok_or("benchmark batch overflow")?;
            }
        }
        let mut samples: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
        for round in 0..9 {
            for offset in 0..4 {
                let mode = (round + offset) % 4;
                samples[mode].push(batch(mode, counts[mode])?.as_secs_f64() * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"width\":{},\"height\":{},\"input_bytes\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",
            a.image.size()[0], a.image.size()[1], input.len(), counts, samples);
    }
    Ok(())
}
'''
    (root / "src/main.rs").write_text(driver, encoding="utf-8")
    # Independently construct DEFLATE fixed-Huffman streams with guaranteed
    # distance-2 or distance-4 overlaps. These are not generated by our codec
    # or the reference crate and thus prove the target branch is exercised.
    import struct
    import zlib

    def manual_fixed_huffman_png(width, distance):
        assert distance in (2, 4)
        byte_count = 4 * width + 1
        literal_prefix = bytes([0]) + (b"AB" if distance == 2 else b"ABCD")
        assert (byte_count - len(literal_prefix)) % 258 == 0
        raw = literal_prefix + (b"AB" if distance == 2 else b"ABCD") * (
            (byte_count - len(literal_prefix)) // (2 if distance == 2 else 4) // 258 * 258
        )
        assert len(raw) == byte_count
        bits = []

        def write_bits(value, count):
            bits.extend((value >> bit) & 1 for bit in range(count))

        def symbol(value):
            if value <= 143:
                code, length = value + 48, 8
            elif value == 256:
                code, length = 0, 7
            elif value == 285:
                code, length = 197, 8
            else:
                raise AssertionError("Only literals, EOB, and length 258 needed")
            reversed_code = int(f"{code:0{length}b}"[::-1], 2)
            write_bits(reversed_code, length)

        write_bits(3, 3)  # BFINAL=1, BTYPE=01 (fixed Huffman)
        for value in literal_prefix:
            symbol(value)
        blocks = (byte_count - len(literal_prefix)) // 258
        for _ in range(blocks):
            symbol(285)  # fixed length 258, no extra length bits
            distance_code = 1 if distance == 2 else 3
            reverse_distance = int(f"{distance_code:05b}"[::-1], 2)
            write_bits(reverse_distance, 5)
        symbol(256)  # end of compressed block
        bits.extend([0] * ((-len(bits)) % 8))
        compressed = bytes(
            sum(bits[shift + i] << i for i in range(8))
            for shift in range(0, len(bits), 8)
        )
        zstream = b"\x78\x01" + compressed + struct.pack(">I", zlib.adler32(raw))
        assert zlib.decompress(zstream) == raw
        ihdr = struct.pack(">IIBBBBB", width, 1, 8, 6, 0, 0, 0)

        def chunk(name, data):
            return (struct.pack(">I", len(data)) + name + data
                    + struct.pack(">I", zlib.crc32(name + data)))
        return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
                + chunk(b"IDAT", zstream) + chunk(b"IEND", b""))

    fixtures = []
    for name, width, distance in [
        ("fixed-distance2", 4064, 2),
        ("fixed-distance4", 4000, 4)
    ]:
        fixture = root / (name + ".png")
        fixture.write_bytes(manual_fixed_huffman_png(width, distance))
        fixtures.append(str(fixture))
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path",
                    str(root / "Cargo.toml")], check=True)
    executable = root / "target/release" / (
        "raipixel-overlap-screen.exe" if sys.platform == "win32" else "raipixel-overlap-screen"
    )
    inputs = [*sys.argv[1:], *fixtures]
    rows = subprocess.check_output([str(executable), *inputs], text=True).splitlines()
    if len(rows) != len(inputs):
        raise ValueError("Unexpected number of benchmark rows")
    cases = []
    for path, row in zip(inputs, rows):
        case = json.loads(row)
        medians = [sorted(samples)[len(samples) // 2] for samples in case["samples_us"]]
        case.update(input=path, median_us=medians,
                    ratios=[time / medians[0] for time in medians],
                    sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(case)
    print(json.dumps({
        "experiment": "safe_deflate_short_overlap_complete_decoder",
        "variants": ["accepted", "distance_2_pattern", "distance_4_pattern", "both"],
        "all_variants_preserve_png_crc_work_and_pixels": True,
        "compiler": subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
        "rustup_toolchain": os.environ.get("RUSTUP_TOOLCHAIN", ""),
        "platform": platform.platform(),
        "source_sha256": hashlib.sha256(SRC.encode()).hexdigest(),
        "variants_sha256": hashes,
        "minimum_batch_ms": 25,
        "rounds": 9,
        "cases": cases
    }))
