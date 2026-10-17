//! Public API fixture used identically for historical and new raster builds.
use raipixel::{Blend, Image, Limits, Rect, Work, png};
use std::{fs, hint::black_box, time::Instant};

fn fixture(side: u32) -> Image {
    let data: Vec<_> = (0..side * side)
        .map(|n| {
            let v = n.wrapping_mul(0x9E37_79B9).rotate_left(9);
            [
                v as u8,
                (v >> 8) as u8,
                (v >> 16) as u8,
                if n % 7 == 0 { 0 } else { 255 },
            ]
        })
        .collect();
    Image::from_pixels(side, side, &data, Limits::default(), &mut Work::default()).unwrap()
}
fn scenario(name: &str) -> (u32, Rect, [i32; 2]) {
    match name {
        "icon" => (16, Rect::new(0, 0, 14, 14).unwrap(), [1, 1]),
        "sprite" => (64, Rect::new(5, 5, 40, 40).unwrap(), [13, 10]),
        "sheet" => (256, Rect::new(0, 0, 224, 224).unwrap(), [16, 16]),
        "large" => (1024, Rect::new(0, 0, 1000, 1000).unwrap(), [12, 12]),
        "clipped" => (256, Rect::new(12, 12, 200, 200).unwrap(), [-100, 40]),
        "upward" => (256, Rect::new(0, 16, 224, 224).unwrap(), [12, 0]),
        _ => panic!("unknown case"),
    }
}
fn task(
    source: &Image,
    rect: Rect,
    at: [i32; 2],
    encode: bool,
) -> (Image, u64, u64, Option<Vec<u8>>) {
    let mut output = source.try_copy(&mut Work::default()).unwrap();
    let mut budget = Work::default();
    let discarded = output
        .stamp_within(rect, at, Blend::Copy, 255, true, &mut budget)
        .unwrap();
    let png_bytes = if encode {
        let mut bytes = Vec::new();
        png::encode_compressed(&output, &mut bytes, &mut budget).unwrap();
        Some(bytes)
    } else {
        None
    };
    (output, budget.remaining(), discarded, png_bytes)
}
fn fingerprint(image: &Image) -> u64 {
    image
        .pixels()
        .iter()
        .flatten()
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        })
}
fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap();
    let mode = args.next().unwrap();
    let iterations: usize = args.next().unwrap().parse().unwrap();
    let dump = args.next();
    assert!(iterations > 0 && (mode == "edit" || mode == "export"));
    let (side, rect, at) = scenario(&name);
    let source = fixture(side);
    let (expected, remaining, discarded, encoded) = task(&source, rect, at, true);
    if let Some(path) = dump {
        fs::write(path, encoded.as_ref().unwrap()).unwrap();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        let (image, _, _, output) = task(&source, rect, at, mode == "export");
        black_box(image);
        black_box(output);
    }
    let elapsed = start.elapsed().as_nanos() / iterations as u128;
    println!(
        "case={name} mode={mode} ns={elapsed} rgba_hash={} remaining={remaining} discarded={discarded} png_bytes={}",
        fingerprint(&expected),
        encoded.unwrap().len()
    );
}
