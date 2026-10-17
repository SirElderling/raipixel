// Independently regrouped Adler/Gailly RFC 1950 sums. Experiment only.
// Dot products use narrow lanes; no architecture intrinsics or unsafe code.
#![forbid(unsafe_code)]
#![allow(dead_code)]
#[path = "../src/checksum.rs"]
mod current;
use std::{hint::black_box, time::{Duration, Instant}};

#[inline]
fn group16(bytes: &[u8; 16]) -> (u16, u16) {
    let (mut sum, mut weighted) = (0u16, 0u16);
    for (i, &x) in bytes.iter().enumerate() {
        sum += u16::from(x);
        weighted += (16 - i) as u16 * u16::from(x);
    }
    // sum <= 4080, weighted <= (16*17/2)*255 = 34680.
    (sum, weighted)
}
fn dot<const W: usize>(data: &[u8]) -> u32 {
    assert_eq!(W % 16, 0);
    let (mut a, mut b) = (1u64, 0u64);
    for block in data.chunks(65_535) {
        let (bulk, tail) = block.as_chunks::<W>();
        for bytes in bulk {
            let (mut sum, mut weighted) = (0u32, 0u32);
            for group in bytes.as_chunks::<16>().0 {
                let (s, w) = group16(group);
                weighted += 16 * sum + u32::from(w);
                sum += u32::from(s);
            }
            b += W as u64 * a + u64::from(weighted);
            a += u64::from(sum);
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
    let fs: [fn(&[u8])->u32;4] = [|d|{let mut a=current::Adler::new();a.update(d);a.value()},dot::<16>,dot::<32>,dot::<128>];
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
