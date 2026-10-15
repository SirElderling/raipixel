//! Microbenchmarks of our own codec stages; no production API or dependency.
//! Filtering below reproduces the current block loop and is checked against
//! the writer's inflated bytes. These isolated times are not additive profiles.
#[allow(dead_code)]
#[path = "../src/checksum.rs"]
mod checksum;
#[allow(dead_code)]
#[path = "../src/deflate.rs"]
mod deflate;
#[allow(dead_code)]
#[path = "../src/limits.rs"]
mod limits;
pub use limits::Work;
use raipixel::Work as CodecWork;
pub use raipixel::{Error, Resource};
use raipixel::{Limits, png};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn filter_block(rgba: &[u8], row_bytes: usize, mut offset: usize, raw: &mut [u8]) {
    let stride = row_bytes + 1;
    let mut filled = 0;
    while filled < raw.len() {
        let row = offset / stride;
        let column = offset % stride;
        if column == 0 {
            raw[filled] = if row == 0 { 1 } else { 2 };
            offset += 1;
            filled += 1;
        } else {
            let take = (stride - column).min(raw.len() - filled);
            let start = row * row_bytes + column - 1;
            let destination = &mut raw[filled..filled + take];
            if row == 0 {
                for (i, value) in destination.iter_mut().enumerate() {
                    let x = start + i;
                    *value = rgba[x].wrapping_sub(if x < 4 { 0 } else { rgba[x - 4] });
                }
            } else {
                for ((value, &sample), &up) in destination
                    .iter_mut()
                    .zip(&rgba[start..start + take])
                    .zip(&rgba[start - row_bytes..start - row_bytes + take])
                {
                    *value = sample.wrapping_sub(up);
                }
            }
            offset += take;
            filled += take;
        }
    }
}
fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: png_stages INPUT.png")?;
    let input = std::fs::read(path)?;
    let image = png::decode(&input, Limits::default(), &mut CodecWork::default())?.image;
    let mut encoded = Vec::new();
    png::encode_compressed(&image, &mut encoded, &mut CodecWork::default())?;
    let mut idat = Vec::new();
    let mut parts = Vec::new();
    for bytes in [&input, &encoded] {
        let mut stream = Vec::new();
        let mut offset = 8;
        while offset < bytes.len() {
            let count = u32::from_be_bytes(bytes[offset..offset + 4].try_into()?) as usize;
            if &bytes[offset + 4..offset + 8] == b"IDAT" {
                stream.extend_from_slice(&bytes[offset + 8..offset + 8 + count]);
                if std::ptr::eq(bytes, &encoded) {
                    parts.push(&bytes[offset + 4..offset + 8 + count]);
                }
            }
            offset += count + 12;
        }
        idat.push(stream);
    }
    let row_bytes = image.size()[0] as usize * 4;
    let raw_size = (row_bytes + 1) * image.size()[1] as usize;
    let rgba = image.pixels().as_flattened();
    let mut raw = vec![0; raw_size];
    for (i, block) in raw.chunks_mut(deflate::BLOCK).enumerate() {
        filter_block(rgba, row_bytes, i * deflate::BLOCK, block);
    }
    assert_eq!(
        deflate::inflate(&idat[1], raw_size, &mut Work::default())
            .unwrap()
            .data,
        raw
    );
    let operation = |mode| -> Result<()> {
        match mode {
            0 => {
                let mut scratch = vec![0; raw_size.min(deflate::BLOCK)];
                for offset in (0..raw_size).step_by(deflate::BLOCK) {
                    let count = (raw_size - offset).min(deflate::BLOCK);
                    filter_block(black_box(rgba), row_bytes, offset, &mut scratch[..count]);
                    black_box(&scratch[..count]);
                }
            }
            1 => {
                let mut a = checksum::Adler::new();
                a.update(black_box(&raw));
                black_box(a.value());
            }
            2 => {
                let mut encoder = deflate::Encoder::new(raw_size.min(deflate::BLOCK))?;
                let mut out = Vec::new();
                let count = raw_size.div_ceil(deflate::BLOCK);
                for (i, block) in raw.chunks(deflate::BLOCK).enumerate() {
                    out.extend_from_slice(encoder.block(black_box(block), i + 1 == count));
                }
                black_box(out);
            }
            3 => {
                for part in &parts {
                    black_box(checksum::crc32(&[black_box(part)]));
                }
            }
            4 => {
                black_box(
                    deflate::inflate(black_box(&idat[0]), raw_size, &mut Work::default()).unwrap(),
                );
            }
            5 => {
                let mut out = Vec::new();
                png::encode_compressed(black_box(&image), &mut out, &mut CodecWork::default())?;
                black_box(out);
            }
            6 => {
                black_box(png::decode(
                    black_box(&input),
                    Limits::default(),
                    &mut CodecWork::default(),
                )?);
            }
            _ => unreachable!(),
        }
        Ok(())
    };
    let batch = |mode, count| -> Result<Duration> {
        let start = Instant::now();
        for _ in 0..count {
            operation(mode)?;
        }
        Ok(start.elapsed())
    };
    let mut counts = [1; 7];
    for (mode, count) in counts.iter_mut().enumerate() {
        while batch(mode, *count)? < Duration::from_millis(25) {
            *count *= 2;
        }
    }
    let mut rounds = [[0.0; 7]; 7];
    for (round, samples) in rounds.iter_mut().enumerate() {
        for n in 0..7 {
            let mode = (round + n) % 7;
            samples[mode] = batch(mode, counts[mode])?.as_secs_f64() * 1e6 / counts[mode] as f64;
        }
    }
    let samples: [[f64; 7]; 7] =
        std::array::from_fn(|mode| std::array::from_fn(|round| rounds[round][mode]));
    let medians: [f64; 7] = samples.map(|mut x| {
        x.sort_by(f64::total_cmp);
        x[3]
    });
    println!(
        "{{\"width\":{},\"height\":{},\"raw_bytes\":{},\"encoded_bytes\":{},\"stages\":[\"filter\",\"adler\",\"deflate_encode\",\"output_idat_crc\",\"inflate_including_adler\",\"encode_compressed\",\"decode\"],\"iterations\":{:?},\"median_us\":{:?},\"samples_us\":{:?}}}",
        image.size()[0],
        image.size()[1],
        raw.len(),
        encoded.len(),
        counts,
        medians,
        samples
    );
    Ok(())
}
