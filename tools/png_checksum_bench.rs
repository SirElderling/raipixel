//! Standalone std-only probe of our own checksum code, not a production API.
//! rustc --edition=2024 -O tools/png_checksum_bench.rs -o /tmp/png-checksum-bench
//! /tmp/png-checksum-bench [INPUT_BYTES]
#[path = "../src/checksum.rs"]
mod checksum;
use std::{hint::black_box, time::Instant};

// Our previous eight-byte weighting, retained only as a benchmark control.
// Source: raipixel 14baddae; no third-party implementation code.
fn previous_adler(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u64, 0u64);
    for block in data.chunks(65_535) {
        let (bulk, tail) = block.as_chunks::<8>();
        for bytes in bulk {
            let [x0, x1, x2, x3, x4, x5, x6, x7] = bytes.map(u64::from);
            b += 8 * a + 8 * x0 + 7 * x1 + 6 * x2 + 5 * x3 + 4 * x4 + 3 * x5 + 2 * x6 + x7;
            a += x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7;
        }
        for &byte in tail {
            a += u64::from(byte);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    ((b << 16) | a) as u32
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = match std::env::args().nth(1) {
        Some(path) => std::fs::read(path)?,
        None => (0..4 * 1024 * 1024).map(|i| (i * 17 + 3) as u8).collect(),
    };
    let mut check = checksum::Adler::new();
    check.update(&data);
    assert_eq!(check.value(), previous_adler(&data));
    let mut samples = [Vec::new(), Vec::new(), Vec::new()];
    for round in 0..7 {
        for n in 0..3 {
            let mode = (n + round) % 3;
            let start = Instant::now();
            for _ in 0..100 {
                if mode == 0 {
                    black_box(checksum::crc32(&[black_box(&data)]));
                } else if mode == 1 {
                    black_box(previous_adler(black_box(&data)));
                } else {
                    let mut adler = checksum::Adler::new();
                    adler.update(black_box(&data));
                    black_box(adler.value());
                }
            }
            samples[mode].push(start.elapsed().as_secs_f64() * 10_000.0);
        }
    }
    let mut medians = [0.0; 3];
    for (median, values) in medians.iter_mut().zip(&samples) {
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        *median = sorted[3];
    }
    println!(
        "{{\"input_bytes\":{},\"iterations_per_batch\":100,\"timing_order\":[\"crc32\",\"adler32_previous\",\"adler32\"],\"samples_us\":{:?},\"median_us\":{:?}}}",
        data.len(),
        samples,
        medians
    );
    Ok(())
}
