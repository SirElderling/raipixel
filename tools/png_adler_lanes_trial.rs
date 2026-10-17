// Independent column grouping of Adler/Gailly RFC 1950 recurrence.
// Diagnostic only; no production selection or third-party source.
#![forbid(unsafe_code)]
#![allow(dead_code)]
#[path = "../src/checksum.rs"]
mod current;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
#[inline(never)]
fn lanes<const W: usize>(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u64, 0u64);
    for block in data.chunks(4096) {
        let (groups, tail) = block.as_chunks::<W>();
        let (mut sums, mut prior) = ([0u32; W], [0u32; W]);
        for bytes in groups {
            for i in 0..W {
                prior[i] += sums[i];
                sums[i] += u32::from(bytes[i]);
            }
        }
        let total: u64 = sums.iter().map(|&x| u64::from(x)).sum();
        let previous: u64 = prior.iter().map(|&x| u64::from(x)).sum();
        let weighted: u64 = sums
            .iter()
            .enumerate()
            .map(|(i, &x)| (W - i) as u64 * u64::from(x))
            .sum();
        b += (groups.len() * W) as u64 * a + W as u64 * previous + weighted;
        a += total;
        for &x in tail {
            a += u64::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    ((b << 16) | a) as u32
}
fn scalar(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
fn main() {
    let fs: [fn(&[u8]) -> u32; 4] = [
        |d| {
            let mut a = current::Adler::new();
            a.update(d);
            a.value()
        },
        lanes::<8>,
        lanes::<16>,
        lanes::<32>,
    ];
    for n in [
        0, 1, 7, 8, 15, 16, 31, 32, 127, 128, 129, 4095, 4096, 4097, 65535, 65536, 131087,
    ] {
        for data in [vec![255; n], (0..n).map(|i| (i * 37 + 11) as u8).collect()] {
            let expected = scalar(&data);
            for f in fs {
                assert_eq!(f(&data), expected, "length {n}");
            }
        }
    }
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let expected = scalar(&data);
        for f in fs {
            assert_eq!(f(&data), expected);
        }
        let batch = |mode: usize, count| {
            let start = Instant::now();
            for _ in 0..count {
                black_box(fs[mode](black_box(&data)));
            }
            start.elapsed()
        };
        let mut counts = [1; 4];
        for (mode, count) in counts.iter_mut().enumerate() {
            while batch(mode, *count) < Duration::from_millis(25) {
                *count *= 2;
            }
        }
        let mut samples: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
        for round in 0..9 {
            for i in 0..4 {
                let mode = (round + i) % 4;
                samples[mode]
                    .push(batch(mode, counts[mode]).as_secs_f64() * 1e6 / counts[mode] as f64);
            }
        }
        println!("{{\"iterations\":{counts:?},\"samples_us\":{samples:?}}}");
    }
}
