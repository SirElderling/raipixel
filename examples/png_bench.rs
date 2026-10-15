//! Small codec-only measurement tool; no benchmark framework or extra dependency.
use raipixel::{Limits, Work, png};
use std::{hint::black_box, num::NonZeroUsize, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: png_bench INPUT.png [ITERATIONS]")?;
    let iterations: NonZeroUsize = args.next().as_deref().unwrap_or("100").parse()?;
    let count = iterations.get();
    let input = std::fs::read(path)?;
    let image = png::decode(&input, Limits::default(), &mut Work::default())?.image;
    // Exclude the first decode, file I/O, process startup and compiler time.
    let start = Instant::now();
    for _ in 0..count {
        black_box(png::decode(
            black_box(&input),
            Limits::default(),
            &mut Work::default(),
        )?);
    }
    let decode_us = start.elapsed().as_secs_f64() * 1_000_000.0 / count as f64;
    let start = Instant::now();
    for _ in 0..count {
        png::encode(
            black_box(&image),
            &mut std::io::sink(),
            &mut Work::default(),
        )?;
    }
    let encode_us = start.elapsed().as_secs_f64() * 1_000_000.0 / count as f64;
    println!(
        "size={:?} input_bytes={} iterations={count} decode_us={decode_us:.3} encode_us={encode_us:.3}",
        image.size(),
        input.len()
    );
    Ok(())
}
