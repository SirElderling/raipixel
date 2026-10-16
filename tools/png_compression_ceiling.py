"""Quantify parity headroom from alternate DEFLATE streams and indexed PNGs.

Run on native Rust 1.99.1 using OUR accepted public PNG decoder/compact writer.
Diagnostic alternatives are made with Python's standard-library zlib and PNG
chunk framing; these are NOT proposals to depend on zlib in production.
No png-crate source is read; every variant must decode to identical RGBA8 in
raipixel (including hidden RGB for transparent pixels).

Usage: python tools/png_compression_ceiling.py INPUT.png [INPUT.png ...]
"""
import binascii
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def chunk(kind: bytes, body: bytes) -> bytes:
    return (len(body).to_bytes(4, "big") + kind + body
            + binascii.crc32(kind + body).to_bytes(4, "big"))

def build_rgba(original: bytes, compressed: bytes) -> bytes:
    return original[:8] + chunk(b"IHDR", original[16:29]) + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")

def parse_idat(blob: bytes) -> bytes:
    assert blob[:8] == b"\x89PNG\r\n\x1a\n"
    pos = 8
    idat = bytearray()
    while pos < len(blob):
        length = int.from_bytes(blob[pos:pos+4], "big")
        kind = blob[pos+4:pos+8]
        body = blob[pos+8:pos+8+length]
        expected = int.from_bytes(blob[pos+8+length:pos+12+length], "big")
        assert binascii.crc32(kind + body) == expected
        if kind == b"IDAT":
            idat.extend(body)
        pos += 12 + length
    assert pos == len(blob)
    return bytes(idat)

def build_palette(width: int, height: int, rgba: bytes, level: int):
    # Dict order is first-occurrence order for fully deterministic, exact RGBA.
    rgba_colors = list(dict.fromkeys(rgba[i:i+4] for i in range(0, len(rgba), 4)))
    if len(rgba_colors) > 256:
        return None
    indices = {rgba: index for index, rgba in enumerate(rgba_colors)}
    data = bytearray()
    for y in range(height):
        data.append(0)  # None row filter; no filtering confound.
        offset = y * width * 4
        for i in range(offset, offset + width * 4, 4):
            data.append(indices[rgba[i:i+4]])
    ihdr = (width.to_bytes(4, "big") + height.to_bytes(4, "big") +
            bytes([8, 3, 0, 0, 0]))
    palette = b"".join(color[:3] for color in rgba_colors)
    alpha = bytes(color[3] for color in rgba_colors)
    trns = chunk(b"tRNS", alpha) if any(value != 255 for value in alpha) else b""
    candidate = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) +
                 chunk(b"PLTE", palette) + trns +
                 chunk(b"IDAT", zlib.compress(data, level)) + chunk(b"IEND", b""))
    return len(rgba_colors), candidate

DRIVER = r'''
#![forbid(unsafe_code)]
use std::io::Write;
use raipixel::{png, Limits, Work};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let image = png::decode(&std::fs::read(&args[2])?,
                            Limits::default(), &mut Work::default())?.image;
    match args[1].as_str() {
        "prepare" => {
            let mut compressed = Vec::new();
            png::encode_compact(&image, &mut compressed, &mut Work::default())?;
            std::fs::write(&args[3], compressed)?;
            let mut output = std::fs::File::create(&args[4])?;
            output.write_all(image.pixels().as_flattened())?;
            println!("{{\"width\":{},\"height\":{}}}", image.size()[0], image.size()[1]);
        }
        "verify" => {
            for path in args.iter().skip(3) {
                let decoded = png::decode(&std::fs::read(path)?,
                    Limits::default(), &mut Work::default())?.image;
                assert_eq!(decoded, image, "RGBA/hidden RGB mismatch in {path}");
            }
            println!("verified {}", args.len() - 3);
        }
        _ => return Err("usage: prepare|verify".into())
    }
    Ok(())
}
'''
with tempfile.TemporaryDirectory(prefix="raipixel-compression-ceiling-") as folder:
    root = Path(folder)
    (root / "src").mkdir()
    (root / "Cargo.toml").write_text(
        '[package]\nname="raipixel-ceiling"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n',
        encoding="utf-8")
    (root / "src/main.rs").write_text(DRIVER, encoding="utf-8")
    subprocess.run(["cargo", "build", "--release", "--offline", "--manifest-path",
                    str(root / "Cargo.toml")], check=True)
    exe = root / "target/release" / (
        "raipixel-ceiling.exe" if sys.platform == "win32" else "raipixel-ceiling")
    cases = []
    for n, path in enumerate(map(Path, sys.argv[1:])):
        original = root / f"original-{n}.png"
        compact = root / f"compact-{n}.png"
        rgba = root / f"rgba-{n}.raw"
        original.write_bytes(path.read_bytes())
        dims = json.loads(subprocess.check_output(
            [str(exe), "prepare", str(original), str(compact), str(rgba)], text=True))
        pixels = rgba.read_bytes()
        assert len(pixels) == dims["width"] * dims["height"] * 4
        compact_data = compact.read_bytes()
        compact_idat = parse_idat(compact_data)
        filtered = zlib.decompress(compact_idat)
        assert len(filtered) == (dims["width"] * 4 + 1) * dims["height"]
        variants = []
        paths = []
        for level in (1, 6, 9):
            stream = zlib.compress(filtered, level)
            reconstructed = build_rgba(compact_data, stream)
            parsed = zlib.decompress(parse_idat(reconstructed))
            assert parsed == filtered
            result = root / f"rgba-level-{level}-{n}.png"
            result.write_bytes(reconstructed)
            paths.append(str(result))
            variants.append({"mode": f"same_rgba_filters_zlib_level{level}",
                             "bytes": len(reconstructed), "idat_bytes": len(stream)})
        palette_count = None
        for level in (1, 6):
            result = build_palette(dims["width"], dims["height"], pixels, level)
            if result is None:
                continue
            palette_count, encoded = result
            result_path = root / f"palette-level-{level}-{n}.png"
            result_path.write_bytes(encoded)
            paths.append(str(result_path))
            variants.append({"mode": f"indexed_rgba_exact_zlib_level{level}",
                             "bytes": len(encoded),
                             "idat_bytes": len(parse_idat(encoded))})
        # Verify *all* alternative PNG encodings in our existing Rust decoder.
        subprocess.run([str(exe), "verify", str(original), *paths],
                       check=True, capture_output=True)
        cases.append({"input": str(path), "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                      "dimensions": dims, "compact_bytes": len(compact_data),
                      "compact_idat_bytes": len(compact_idat),
                      "palette_colors": palette_count,
                      "variants": [{**v, "size_over_compact": v["bytes"] / len(compact_data)}
                                   for v in variants],
                      "all_variants_identical_exact_rgba": True})
    print(json.dumps({
        "experiment": "compression_strategy_and_palette_parity_headroom",
        "purpose": "Measure potential size gains, NOT equivalent Rust implementation speed",
        "note": "zlib is Python stdlib diagnostic only; reference png crate source not inspected",
        "compiler": subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
        "platform": platform.platform(),
        "python_zlib": zlib.ZLIB_RUNTIME_VERSION,
        "rustup_toolchain": os.environ.get("RUSTUP_TOOLCHAIN", ""),
        "driver_sha256": hashlib.sha256(DRIVER.encode()).hexdigest(),
        "cases": cases
    }))
