//! Standalone std-only probe of our own checksum code, not a production API.
//! rustc --edition=2024 -O tools/png_checksum_bench.rs -o /tmp/png-checksum-bench
//! /tmp/png-checksum-bench [INPUT_BYTES]
#[path = "../src/checksum.rs"]
mod checksum;
use std::{hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = match std::env::args().nth(1) {
        Some(path) => std::fs::read(path)?,
        None => (0..4 * 1024 * 1024).map(|i| (i * 17 + 3) as u8).collect(),
    };
    let mut samples = [Vec::new(), Vec::new()];
    for round in 0..7 {
        for mode in [round % 2, (round + 1) % 2] {
            let start = Instant::now();
            for _ in 0..100 {
                if mode == 0 {
                    black_box(checksum::crc32(&[black_box(&data)]));
                } else {
                    let mut adler = checksum::Adler::new();
                    adler.update(black_box(&data));
                    black_box(adler.value());
                }
            }
            samples[mode].push(start.elapsed().as_secs_f64() * 10_000.0);
        }
    }
    let mut medians = [0.0; 2];
    for (median, values) in medians.iter_mut().zip(&samples) {
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        *median = sorted[3];
    }
    println!(
        "{{\"input_bytes\":{},\"iterations_per_batch\":100,\"timing_order\":[\"crc32\",\"adler32\"],\"samples_us\":{:?},\"median_us\":{:?}}}",
        data.len(),
        samples,
        medians
    );
    Ok(())
}
