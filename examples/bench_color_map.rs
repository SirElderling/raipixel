//! Reference-only timing trial for exact RGB recolor strategies.
//! No production dependency on this file; do not treat microbenchmarks as universal wins.
use raipixel::{Image, Limits, Pixel, Work};
use std::hint::black_box;
use std::time::{Duration, Instant};

fn palette(i: usize) -> Pixel {
    let n = i as u8;
    [n, n.wrapping_mul(17), n.wrapping_mul(37), 255]
}
fn sample(width: usize, height: usize, colors: usize) -> Vec<Pixel> {
    (0..width * height)
        .map(|i| {
            let x = i % width;
            let y = i / width;
            let mut p = palette((x * 7 + y * 13 + (x / 8) * 3) % colors);
            // Transparency preserves nonzero RGB in every workload.
            p[3] = ((i * 37 + 11) & 255) as u8;
            p
        })
        .collect()
}
fn mappings(colors: usize) -> Vec<(Pixel, Pixel)> {
    (0..colors)
        .map(|i| {
            let mut p = palette(i);
            p[3] = 0;
            let mut to = palette(colors - i - 1);
            to[3] = 0;
            (p, to)
        })
        .collect()
}

fn linear(original: &[Pixel], pairs: &[(Pixel, Pixel)]) -> Vec<Pixel> {
    let mut pixels = original.to_vec();
    for pixel in &mut pixels {
        if let Some((_, dest)) = pairs.iter().find(|(from, _)| from[..3] == pixel[..3]) {
            pixel[..3].copy_from_slice(&dest[..3]);
        }
    }
    pixels
}

fn binary(original: &[Pixel], width: usize, pairs: &[(Pixel, Pixel)]) -> Image {
    let mut budget = Work::default();
    let mut image = Image::from_pixels(
        width as u32,
        (original.len() / width) as u32,
        original,
        Limits::default(),
        &mut budget,
    )
    .unwrap();
    image.map_colors(pairs, true, None, &mut budget).unwrap();
    image
}

fn hash(original: &[Pixel], pairs: &[(Pixel, Pixel)]) -> Vec<Pixel> {
    const SIZE: usize = 2048;
    const EMPTY: u32 = u32::MAX;
    let mut keys = [EMPTY; SIZE];
    let mut values = [[0u8; 3]; SIZE];
    for &(from, to) in pairs {
        let key = u32::from_be_bytes([0, from[0], from[1], from[2]]);
        let mut index = (key.wrapping_mul(0x9E37_79B1) as usize) & (SIZE - 1);
        while keys[index] != EMPTY {
            index = (index + 1) & (SIZE - 1);
        }
        keys[index] = key;
        values[index] = [to[0], to[1], to[2]];
    }
    let mut pixels = original.to_vec();
    for pixel in &mut pixels {
        let key = u32::from_be_bytes([0, pixel[0], pixel[1], pixel[2]]);
        let mut index = (key.wrapping_mul(0x9E37_79B1) as usize) & (SIZE - 1);
        while keys[index] != EMPTY {
            if keys[index] == key {
                pixel[..3].copy_from_slice(&values[index]);
                break;
            }
            index = (index + 1) & (SIZE - 1);
        }
    }
    pixels
}

fn execute(method: usize, original: &[Pixel], width: usize, pairs: &[(Pixel, Pixel)]) {
    match method {
        0 => { black_box(linear(original, pairs)); }
        1 => { black_box(binary(original, width, pairs)); }
        _ => { black_box(hash(original, pairs)); }
    }
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.total_cmp(b));
    samples[samples.len() / 2]
}

fn bench_case(width: usize, height: usize, colors: usize) {
    let original = sample(width, height, colors);
    let pairs = mappings(colors);
    let reference = linear(&original, &pairs);
    assert_eq!(binary(&original, width, &pairs).pixels(), reference.as_slice(), "binary mismatch");
    assert_eq!(hash(&original, &pairs), reference, "hash mismatch");
    let names = ["linear", "native_map_colors", "fixed_hash"];
    let mut batches: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
    // Calibrate batches separately, then rotate order over nine rounds.
    let mut repetitions = [1usize; 3];
    for (method, count) in repetitions.iter_mut().enumerate() {
        loop {
            let start = Instant::now();
            for _ in 0..*count {
                execute(method, &original, width, &pairs);
            }
            if start.elapsed() >= Duration::from_millis(20) || *count >= 1024 {
                break;
            }
            *count *= 2;
        }
    }
    for round in 0..9 {
        for offset in 0..3 {
            let method = (round + offset) % 3;
            let start = Instant::now();
            for _ in 0..repetitions[method] {
                execute(method, &original, width, &pairs);
            }
            batches[method].push(
                start.elapsed().as_secs_f64() * 1_000_000.0 / repetitions[method] as f64,
            );
        }
    }
    for method in 0..3 {
        println!(
            "{}x{} colors={} method={} median_us={:.2} repetitions={}",
            width,
            height,
            colors,
            names[method],
            median(std::mem::take(&mut batches[method])),
            repetitions[method],
        );
    }
}

fn main() {
    println!("raipixel exact-RGB color-map strategies, release-mode paired machine test");
    for (width, height, colors) in [
        (64, 64, 8),
        (512, 512, 8),
        (512, 512, 64),
        (256, 256, 256),
    ] {
        bench_case(width, height, colors);
    }
}
