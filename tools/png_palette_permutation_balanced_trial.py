"""C32: exhaustive exact-RGBA palette permutation screen with integrated balanced DEFLATE.

For images with at most five exact RGBA colors, try every encounter-palette
permutation (up to 120), with all three opt-in balanced filters. Confirm each
candidate by the independent public png 0.18.1 decoder, including hidden RGB,
against the existing encoder's output bytes. No accepted source is modified;
the candidate is instrumented in a temporary copy of our own safe Rust.

Run: python tools/png_palette_permutation_balanced_trial.py INPUT.png ...
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


def one(source, old, new):
    count = source.count(old)
    if count != 1:
        raise ValueError(f"unstable scratch anchor {old[:80]!r}: {count}")
    return source.replace(old, new)


PALETTE_MUTATION = r"""
    if let Some(order) = palette_order {
        if order.len() != exact.colors.len() {
            return Err(Error::Chunk);
        }
        // This is a small-palette scratch screen, so a fixed stack array
        // avoids introducing unmetered additional heap allocations.
        let mut seen = [false; 256];
        let mut ordered = [[0u8; 4]; 256];
        for (destination, &source) in order.iter().enumerate() {
            if source >= exact.colors.len() || seen[source] {
                return Err(Error::Chunk);
            }
            seen[source] = true;
            ordered[destination] = exact.colors[source];
        }
        for (destination, sample) in exact.colors.iter_mut().enumerate() {
            *sample = ordered[destination];
        }
        exact.slots.fill(u16::MAX);
        for (index, &sample) in exact.colors.iter().enumerate() {
            let mut slot = palette_slot(sample);
            while exact.slots[slot] != u16::MAX {
                slot = (slot + 1) & 4095;
            }
            exact.slots[slot] = index as u16;
        }
    }
"""


PUBLIC_WRAPPER = r"""
/// Scratch-only exact-RGBA palette permutation benchmark, not product API.
pub fn encode_indexed_balanced_palette_trial(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
    order: &[usize],
) -> Result<bool, Error> {
    encode_balanced_inner(image, output, work, None, Some(order))
}
"""


def instrument_quality(source):
    source = one(
        source,
        "    encode_balanced_inner(image, output, work, None)",
        "    encode_balanced_inner(image, output, work, None, None)",
    )
    source = one(
        source,
        "    injected_allocation_pass: Option<usize>,\n) -> Result<bool, Error> {",
        "    injected_allocation_pass: Option<usize>,\n"
        "    palette_order: Option<&[usize]>,\n) -> Result<bool, Error> {",
    )
    source = one(
        source,
        "    let Some(exact) = discover_palette(image, work)? else {",
        "    let Some(mut exact) = discover_palette(image, work)? else {",
    )
    source = one(
        source,
        "    let mut best: Option<Vec<u8>> = None;",
        PALETTE_MUTATION + "    let mut best: Option<Vec<u8>> = None;",
    )
    source = one(
        source,
        "                Some(pass),\n            ).unwrap_err();",
        "                Some(pass),\n                None,\n            ).unwrap_err();",
    )
    source = one(
        source,
        "            &image, &mut normal, &mut Work::default(), None\n",
        "            &image, &mut normal, &mut Work::default(), None, None\n",
    )
    source = one(
        source,
        "\n#[cfg(test)]\nmod fault_injection_tests {",
        "\n" + PUBLIC_WRAPPER + "\n#[cfg(test)]\nmod fault_injection_tests {",
    )
    return source


DRIVER = r'''
#![forbid(unsafe_code)]
use std::{io::Cursor, path::Path};
use raipixel::{Limits, Work};
type Failure = Box<dyn std::error::Error>;

fn public_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), Failure> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    decoder.ignore_checksums(false);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0u8; reader.output_buffer_size().ok_or("PNG decode buffer")?];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    reader.finish()?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer.chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255]).collect(),
        _ => return Err("reference decoder did not expand indexed output".into()),
    };
    Ok((info.width, info.height, rgba))
}

fn all_orders(n: usize) -> Vec<Vec<usize>> {
    fn visit(depth: usize, chosen: &mut [bool], partial: &mut Vec<usize>,
             result: &mut Vec<Vec<usize>>) {
        if depth == chosen.len() {
            result.push(partial.clone());
            return;
        }
        for index in 0..chosen.len() {
            if chosen[index] { continue; }
            chosen[index] = true;
            partial.push(index);
            visit(depth + 1, chosen, partial, result);
            partial.pop();
            chosen[index] = false;
        }
    }
    let mut orders = Vec::new();
    visit(0, &mut vec![false; n], &mut Vec::new(), &mut orders);
    orders
}

fn main() -> Result<(), Failure> {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let original = raipixel::png::decode(
            &std::fs::read(path)?, Limits::default(), &mut Work::default()
        )?.image;
        let [width, height] = original.size();
        let mut colors = Vec::<[u8; 4]>::new();
        for &pixel in original.pixels() {
            if !colors.contains(&pixel) {
                colors.push(pixel);
                if colors.len() > 5 { break; }
            }
        }
        let name = path.file_name().ok_or("missing name")?.to_string_lossy();
        if colors.len() > 5 {
            println!("{{\"name\":{:?},\"tested\":false,\"colors\":\"more_than_5\"}}", name);
            continue;
        }
        let candidate_image = candidate::Image::from_pixels(
            width, height, original.pixels(), candidate::Limits::default(),
            &mut candidate::Work::default()
        )?;
        let mut accepted = Vec::new();
        assert!(raipixel::png::encode_indexed_balanced(
            &original, &mut accepted, &mut Work::default()
        )?, "baseline unsupported unexpectedly");
        let mut best_size = accepted.len();
        let mut best_order = (0..colors.len()).collect::<Vec<usize>>();
        let mut best = accepted.clone();
        let orders = all_orders(colors.len());
        assert!(orders.len() <= 120, "permutation cap exceeded");
        let mut wins = 0usize;
        let mut checks = 0usize;
        for (rank, order) in orders.iter().enumerate() {
            let mut bytes = Vec::new();
            assert!(candidate::png::encode_indexed_balanced_palette_trial(
                &candidate_image, &mut bytes, &mut candidate::Work::default(), order
            )?, "candidate unsupported unexpectedly");
            if rank == 0 {
                assert_eq!(bytes, accepted, "identity palette changed baseline bytes");
            }
            let (x, y, rgba) = public_rgba(&bytes)?;
            assert_eq!([x, y], [width, height]);
            assert_eq!(rgba.as_slice(), original.pixels().as_flattened(),
                       "public PNG decoder found RGBA loss");
            assert_eq!(raipixel::png::decode(
                &bytes, Limits::default(), &mut Work::default()
            )?.image, original, "own PNG decoder found RGBA loss");
            checks += 1;
            if bytes.len() < best_size {
                best_size = bytes.len();
                best_order = order.clone();
                best = bytes;
                wins += 1;
            }
        }
        let mut repeated = Vec::new();
        assert!(candidate::png::encode_indexed_balanced_palette_trial(
            &candidate_image, &mut repeated, &mut candidate::Work::default(), &best_order
        )?);
        assert_eq!(repeated, best, "best permutation not deterministic");
        let mut corrupted = best.clone();
        corrupted[29] ^= 1;
        assert!(matches!(raipixel::png::decode(
            &corrupted, Limits::default(), &mut Work::default()
        ), Err(raipixel::png::Error::Crc)), "CRC corruption accepted");
        println!("{{\"name\":{:?},\"tested\":true,\"width\":{},\"height\":{},\
                  \"colors\":{},\"permutations\":{},\"baseline_bytes\":{},\
                  \"best_bytes\":{},\"strict_improvements\":{},\"best_order\":{:?},\
                  \"independent_rgba_decodes\":{},\"deterministic_and_crc\":true}}",
                 name, width, height, colors.len(), orders.len(), accepted.len(),
                 best_size, wins, best_order, checks);
    }
    Ok(())
}
'''


def main():
    with tempfile.TemporaryDirectory(prefix="raipixel-palette-permutation-") as temp:
        folder = Path(temp)
        scratch = folder / "candidate"
        shutil.copytree(ROOT / "src", scratch / "src")
        (scratch / "Cargo.toml").write_text(
            '[package]\nname="raipixel_palette_candidate"\n'
            'version="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n'
            '[features]\nexperimental-quality=[]\n'
            '[lints.rust]\nunsafe_code="forbid"\n',
            encoding="utf-8",
        )
        source = (ROOT / "src/png/quality.rs").read_text(encoding="utf-8")
        modified = instrument_quality(source)
        (scratch / "src/png/quality.rs").write_text(modified, encoding="utf-8")
        png_source = (scratch / "src/png.rs").read_text(encoding="utf-8")
        png_source = one(
            png_source,
            "pub use quality::encode_indexed_balanced;",
            "pub use quality::encode_indexed_balanced;\n"
            '#[cfg(feature = "experimental-quality")]\n'
            "pub use quality::encode_indexed_balanced_palette_trial;",
        )
        (scratch / "src/png.rs").write_text(png_source, encoding="utf-8")
        (folder / "src").mkdir()
        (folder / "Cargo.toml").write_text(
            '[package]\nname="png-comparison"\nversion="0.0.0"\n'
            'edition="2024"\npublish=false\n'
            '[dependencies]\npng="=0.18.1"\n'
            'raipixel={path=' + json.dumps(str(ROOT)) +
            ',features=["experimental-quality"]}\n'
            'candidate={package="raipixel_palette_candidate",path=' +
            json.dumps(str(scratch)) + ',features=["experimental-quality"]}\n',
            encoding="utf-8",
        )
        shutil.copyfile(ROOT / "benchmarks/png/reference.lock", folder / "Cargo.lock")
        (folder / "src/main.rs").write_text(DRIVER, encoding="utf-8")
        subprocess.run(
            ["cargo", "build", "--release", "--manifest-path",
             str(folder / "Cargo.toml")], check=True
        )
        exe = folder / "target/release" / (
            "png-comparison.exe" if sys.platform == "win32" else "png-comparison"
        )
        output = subprocess.check_output(
            [str(exe), *sys.argv[1:]], text=True
        ).splitlines()
        if len(output) != len(sys.argv[1:]):
            raise ValueError("missing palette case result")
        cases = []
        for path, line in zip(sys.argv[1:], output):
            case = json.loads(line)
            case["input_sha256"] = hashlib.sha256(Path(path).read_bytes()).hexdigest()
            cases.append(case)
        print(json.dumps({
            "experiment": "C32_balanced_exhaustive_exact_rgba_palette_permutation",
            "platform": platform.platform(),
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
            "candidate_sha256": hashlib.sha256(modified.encode()).hexdigest(),
            "max_palette_colors": 5,
            "all_tested_candidates_independently_decoded_public_png_0_18_1": True,
            "production_code_changed": False,
            "cases": cases,
        }))


if __name__ == "__main__":
    main()
