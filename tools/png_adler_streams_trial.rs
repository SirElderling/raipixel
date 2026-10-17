// Independently regrouped Adler/Gailly RFC 1950 sums. Experiment only.
// Interleaved packed streams; no architecture intrinsics or unsafe code.
#![forbid(unsafe_code)]
#![allow(dead_code)]
#[path = "../src/checksum.rs"]
mod current;
use std::{hint::black_box, time::{Duration, Instant}};

#[inline]
fn reduce(even: u64, odd: u64, prior_even: u64, prior_odd: u64) -> (u64, u64, u64) {
    let sums = even + odd;
    let sum = sums.wrapping_mul(0x0001_0001_0001_0001) >> 48;
    let odd_sum = odd.wrapping_mul(0x0001_0001_0001_0001) >> 48;
    let prior = prior_even + prior_odd;
    let pairs = (prior & 0x0000_ffff_0000_ffff) + ((prior >> 16) & 0x0000_ffff_0000_ffff);
    let prior_sum = (pairs & 0xffff_ffff) + (pairs >> 32);
    let weighted = 4 * (sums & 65535) + 3 * ((sums >> 16) & 65535)
        + 2 * ((sums >> 32) & 65535) + (sums >> 48);
    (sum, prior_sum, 2 * weighted - odd_sum)
}
fn packed<const LANES: usize>(data: &[u8]) -> u32 {
    assert!(LANES > 0 && 16 % LANES == 0);
    let (mut a, mut b) = (1u64, 0u64);
    for block in data.chunks(65_535) {
        let (bulk, tail) = block.as_chunks::<128>();
        for bytes in bulk {
            let (mut even, mut odd, mut pe, mut po) = ([0u64; LANES], [0u64; LANES], [0u64; LANES], [0u64; LANES]);
            for words in bytes.chunks_exact(8 * LANES) {
                for lane in 0..LANES {
                    let word = u64::from_le_bytes(words[8*lane..8*lane+8].try_into().unwrap());
                    pe[lane] += even[lane]; po[lane] += odd[lane];
                    even[lane] += word & 0x00ff_00ff_00ff_00ff;
                    odd[lane] += (word >> 8) & 0x00ff_00ff_00ff_00ff;
                }
            }
            b += 128 * a;
            for lane in 0..LANES {
                let (sum, prior, weighted) = reduce(even[lane], odd[lane], pe[lane], po[lane]);
                b += (8 * LANES) as u64 * prior + (8 * (LANES - 1 - lane)) as u64 * sum + weighted;
                a += sum;
            }
        }
        for &x in tail { a += u64::from(x); b += a; }
        a %= 65521; b %= 65521;
    }
    ((b << 16) | a) as u32
}
fn scalar(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data { a = (a + u32::from(x)) % 65521; b = (b + a) % 65521; }
    (b << 16) | a
}
fn main() {
    let fs: [fn(&[u8])->u32;4] = [|d|{let mut a=current::Adler::new();a.update(d);a.value()},packed::<1>,packed::<2>,packed::<4>];
    for n in [0,1,15,16,17,31,32,33,127,128,129,4095,4096,4097,65534,65535,65536,131087] {
        for data in [vec![255;n], (0..n).map(|i|(i*37+11) as u8).collect()] {
            let expected=scalar(&data);for f in fs {assert_eq!(f(&data),expected,"length {n}");}
        }
    }
    for position in 0..128 {
        let mut data=[0u8;128];data[position]=255;
        for f in fs {assert_eq!(f(&data),scalar(&data));}
    }
    let data: Vec<u8>=(0..4*1024*1024).map(|i|(i*17+3)as u8).collect();
    let expected=scalar(&data);for f in fs {assert_eq!(f(&data),expected);}
    let batch=|mode:usize,count|{let start=Instant::now();for _ in 0..count{black_box(fs[mode](black_box(&data)));}start.elapsed()};
    let mut counts=[1;4];for(mode,count)in counts.iter_mut().enumerate(){while batch(mode,*count)<Duration::from_millis(25){*count*=2;}}
    let mut samples:[Vec<f64>;4]=std::array::from_fn(|_|Vec::new());
    for round in 0..9{for n in 0..4{let mode=(round+n)%4;samples[mode].push(batch(mode,counts[mode]).as_secs_f64()*1e6/counts[mode]as f64);}}
    println!("{{\"bytes\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",data.len(),counts,samples);
}
