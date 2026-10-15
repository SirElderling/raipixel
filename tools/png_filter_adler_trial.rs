//! Optimistic Up-filter/Adler fusion screen, not a production implementation.
//! Omits row markers/Sub/boundaries: reject if it loses even without that cost.
//! Credits/derivation: benchmarks/png/algorithm-notes.md (PNG WG, Adler/Gailly,
//! Deutsch; fusion experiment concept: Joglekar, Kounavis & Berry, FAST 2005).
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "../src/checksum.rs"]
mod checksum;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
const BLOCK: usize = 65_535;
const MASK: u64 = 0x00ff_00ff_00ff_00ff;
const GUARD: u64 = 0x0100_0100_0100_0100;

fn fused(samples: &[u8], previous: &[u8], output: &mut [u8], a: &mut u64, b: &mut u64) {
    let (bulk, tail) = output.as_chunks_mut::<128>();
    for (index, group) in bulk.iter_mut().enumerate() {
        let start = index * 128;
        let (mut even, mut odd, mut pe, mut po) = (0u64, 0u64, 0u64, 0u64);
        for (j, destination) in group.as_chunks_mut::<8>().0.iter_mut().enumerate() {
            let p = start + j * 8;
            let x = u64::from_le_bytes(samples[p..p + 8].try_into().unwrap());
            let y = u64::from_le_bytes(previous[p..p + 8].try_into().unwrap());
            // A guard bit per 16-bit lane prevents cross-byte borrow.
            let e = ((x & MASK | GUARD) - (y & MASK)) & MASK;
            let o = (((x >> 8) & MASK | GUARD) - ((y >> 8) & MASK)) & MASK;
            *destination = (e | (o << 8)).to_le_bytes();
            pe += even;
            po += odd;
            even += e;
            odd += o;
        }
        *b += 128 * *a;
        for i in 0..4 {
            let shift = i * 16;
            let e = (even >> shift) & 65535;
            let o = (odd >> shift) & 65535;
            let prior = ((pe >> shift) & 65535) + ((po >> shift) & 65535);
            *b += 8 * prior + (8 - 2 * i) * e + (7 - 2 * i) * o;
            *a += e + o;
        }
    }
    let start = samples.len() - tail.len();
    for ((out, &x), &y) in tail
        .iter_mut()
        .zip(&samples[start..])
        .zip(&previous[start..])
    {
        *out = x.wrapping_sub(y);
        *a += u64::from(*out);
        *b += *a;
    }
    *a %= 65_521;
    *b %= 65_521;
}
fn baseline(samples: &[u8], previous: &[u8], output: &mut [u8]) {
    for ((out, &x), &y) in output.iter_mut().zip(samples).zip(previous) {
        *out = x.wrapping_sub(y);
    }
}
fn main() {
    let samples: Vec<u8> = (0..4 * 1024 * 1024).map(|i| (i * 17 + 3) as u8).collect();
    let previous: Vec<u8> = (0..samples.len())
        .map(|i| (i * 71 + i / 113) as u8)
        .collect();
    let mut expected = vec![0; samples.len()];
    baseline(&samples, &previous, &mut expected);
    let mut actual = vec![0; samples.len()];
    let (mut a, mut b) = (1, 0);
    for (i, out) in actual.chunks_mut(BLOCK).enumerate() {
        let start = i * BLOCK;
        fused(
            &samples[start..start + out.len()],
            &previous[start..start + out.len()],
            out,
            &mut a,
            &mut b,
        );
    }
    assert_eq!(actual, expected);
    let mut check = checksum::Adler::new();
    check.update(&expected);
    assert_eq!(((b << 16) | a) as u32, check.value());
    let operation = |mode| {
        let mut scratch = vec![0; BLOCK];
        let (mut a, mut b) = (1, 0);
        let mut adler = checksum::Adler::new();
        for (i, x) in samples.chunks(BLOCK).enumerate() {
            let y = &previous[i * BLOCK..i * BLOCK + x.len()];
            let out = &mut scratch[..x.len()];
            if mode == 0 {
                baseline(black_box(x), black_box(y), out);
                adler.update(out);
            } else {
                fused(black_box(x), black_box(y), out, &mut a, &mut b);
            }
            black_box(out);
        }
        black_box(if mode == 0 {
            adler.value()
        } else {
            ((b << 16) | a) as u32
        });
    };
    let batch = |mode, count| {
        let start = Instant::now();
        for _ in 0..count {
            operation(mode);
        }
        start.elapsed()
    };
    let mut counts = [1; 2];
    for (mode, count) in counts.iter_mut().enumerate() {
        while batch(mode, *count) < Duration::from_millis(25) {
            *count *= 2;
        }
    }
    let mut samples_us = [Vec::new(), Vec::new()];
    for round in 0..9 {
        for n in 0..2 {
            let mode = (round + n) % 2;
            samples_us[mode]
                .push(batch(mode, counts[mode]).as_secs_f64() * 1e6 / counts[mode] as f64);
        }
    }
    let medians: Vec<_> = samples_us
        .iter()
        .map(|s| {
            let mut v = s.clone();
            v.sort_by(f64::total_cmp);
            v[4]
        })
        .collect();
    println!(
        "{{\"input_bytes\":{},\"modes\":[\"separate\",\"fused\"],\"iterations\":{:?},\"median_us\":{:?},\"samples_us\":{:?}}}",
        samples.len(),
        counts,
        medians,
        samples_us
    );
}
