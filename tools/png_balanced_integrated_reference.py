"""Paired test of the *integrated* feature-gated PNG encoder vs png 0.18.1.

No external production dependencies: png 0.18.1 is used only in this
temporary test Cargo project. The reference writer uses its public API.
All candidate outputs are independently decoded to exact RGBA, including
the RGB components under alpha zero. The 9-round speed comparison includes
each full public encode call but excludes initial PNG input decoding.
Usage: python tools/png_balanced_integrated_reference.py ICON.png GENERATED.png ORIGINAL.png
"""
import binascii
import hashlib
import json
import os
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FROZEN = {
    "icon-64-compressed.png": 158,
    "sheet-1024-compressed.png": 1653,
    "comparison-sheet-1024.png": 3351,
    "art-diagonal-16-257x129.png": 517,
    "art-band-16-512x128.png": 445,
    "indexed-case-3-1024x300.png": 730,
}


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(
        ">I", binascii.crc32(kind + data)
    )


def write_rgba_png(folder, name, width, height, pixels):
    stride = width * 4
    raw = b"".join(b"\x00" + pixels[y * stride : (y + 1) * stride] for y in range(height))
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    data = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))
    path = folder / name
    path.write_bytes(data)
    return path


def art_fixture(folder, name, width, height, seed, diagonal):
    palette = [
        (i & 255, (i * 37 + seed * 3) & 255, (i * 83 + seed * 7) & 255,
         (0, 64, 128, 255)[(i + seed) % 4])
        for i in range(16)
    ]
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            index = ((x + y) // 7 if diagonal else y // 4 + x // 73) % 16
            pixels.extend(palette[index])
    return write_rgba_png(folder, name, width, height, pixels)


def cross_block_fixture(folder):
    pixels = bytearray()
    for y in range(300):
        for x in range(1024):
            index = (x * 7 + y * 11) % 3
            pixels.extend((index, (index * 41 + 7) & 255,
                           (index * 73 + 17) & 255, (0, 127, 255)[index]))
    return write_rgba_png(folder, "indexed-case-3-1024x300.png", 1024, 300, pixels)


def main():
    with tempfile.TemporaryDirectory(prefix="raipixel-balanced-integrated-") as temp:
        folder = Path(temp)
        (folder / "src").mkdir()
        manifest = (
            '[package]\nname="png-comparison"\nversion="0.0.0"\nedition="2024"\npublish=false\n'
            '[dependencies]\npng="=0.18.1"\nraipixel={path='
            + json.dumps(str(ROOT)) + ',features=["experimental-quality"]}\n'
        )
        (folder / "Cargo.toml").write_text(manifest, encoding="utf-8")
        shutil.copyfile(ROOT / "benchmarks/png/reference.lock", folder / "Cargo.lock")
        reference = (ROOT / "tools/png_indexed_reference.rs").read_text(encoding="utf-8")
        old = "raipixel::png::encode_indexed("
        if reference.count(old) != 2:
            raise RuntimeError("reference driver public-API encode call count changed")
        reference = reference.replace(old, "raipixel::png::encode_indexed_balanced(")
        (folder / "src/main.rs").write_text(reference, encoding="utf-8")
        subprocess.run(
            ["cargo", "build", "--release", "--locked",
             "--manifest-path", str(folder / "Cargo.toml")], check=True
        )
        exe = folder / "target/release" / (
            "png-comparison.exe" if sys.platform == "win32" else "png-comparison"
        )
        paths = list(map(Path, sys.argv[1:]))
        paths.append(art_fixture(
            folder, "art-diagonal-16-257x129.png", 257, 129, 4, True
        ))
        paths.append(art_fixture(
            folder, "art-band-16-512x128.png", 512, 128, 5, False
        ))
        paths.append(cross_block_fixture(folder))
        output = subprocess.check_output(
            [str(exe), *map(str, paths)], text=True
        ).splitlines()
        if len(output) != len(paths):
            raise RuntimeError("reference driver missed fixture output")
        cases = []
        for path, line in zip(paths, output):
            case = json.loads(line)
            case["name"] = path.name
            case["input_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            if not case["supported"]:
                raise AssertionError(f"indexed fixture unsupported: {path.name}")
            expected = FROZEN[path.name]
            if case["bytes"][0] != expected:
                raise AssertionError(
                    f"integrated balanced byte-size regression on {path.name}: "
                    f"got {case['bytes'][0]}, expected {expected}"
                )
            samples = case["samples_us"]
            case["median_us"] = [sorted(x)[len(x)//2] for x in samples]
            case["reference_default_over_balanced"] = (
                case["median_us"][2] / case["median_us"][0]
            )
            cases.append(case)
        print(json.dumps({
            "experiment": "integrated_balanced_png_vs_public_png_0_18_1",
            "compiler": subprocess.check_output(
                ["rustc", "--version"], text=True
            ).strip(),
            "platform": platform.platform(),
            "toolchain": os.environ.get("RUSTUP_TOOLCHAIN", ""),
            "public_reference": "png 0.18.1, public API, default and Fast",
            "feature": "experimental-quality",
            "all_outputs_independently_decoded_exact_rgba": True,
            "locked_reference_package": True,
            "calibration_ms": 25,
            "rounds": 9,
            "cases": cases,
        }))


if __name__ == "__main__":
    main()
