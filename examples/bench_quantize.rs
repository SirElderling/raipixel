//! A/B trial for exact squared-RGB quantization: direct scan vs bounded color cache.
//! cargo run --release --locked --offline --example bench_quantize
use raipixel::{Image, Limits, Pixel, Work, png};
use std::{hint::black_box, time::Instant};

const CACHE_SLOTS: usize = 4096;
const MAX_PROBES: usize = 8;

fn nearest(rgb: [u8; 3], palette: &[[u8; 3]]) -> usize {
    let mut chosen = 0;
    let mut closest = u32::MAX;
    for (index, &entry) in palette.iter().enumerate() {
        let dr = i32::from(rgb[0]) - i32::from(entry[0]);
        let dg = i32::from(rgb[1]) - i32::from(entry[1]);
        let db = i32::from(rgb[2]) - i32::from(entry[2]);
        let distance = (dr * dr + dg * dg + db * db) as u32;
        if distance < closest {
            closest = distance;
            chosen = index;
        }
    }
    chosen
}

fn direct(input: &[Pixel], palette: &[[u8; 3]]) -> Vec<Pixel> {
    let mut output = input.to_vec();
    for sample in &mut output {
        if sample[3] != 0 {
            let index = nearest([sample[0], sample[1], sample[2]], palette);
            sample[..3].copy_from_slice(&palette[index]);
        }
    }
    output
}

fn cached(input: &[Pixel], palette: &[[u8; 3]]) -> Vec<Pixel> {
    let mut output = input.to_vec();
    let mut cache = [None::<([u8; 3], u8)>; CACHE_SLOTS];
    for sample in &mut output {
        if sample[3] == 0 {
            continue;
        }
        let key = [sample[0], sample[1], sample[2]];
        let packed = u32::from_be_bytes([0, key[0], key[1], key[2]]);
        let mut slot = (packed.wrapping_mul(0x9E37_79B1) as usize) & (CACHE_SLOTS - 1);
        let mut hit = None;
        let mut free = None;
        for _ in 0..MAX_PROBES {
            match cache[slot] {
                Some((rgb, index)) if rgb == key => {
                    hit = Some(usize::from(index));
                    break;
                }
                None => {
                    free = Some(slot);
                    break;
                }
                _ => slot = (slot + 1) & (CACHE_SLOTS - 1),
            }
        }
        let index = match hit {
            Some(index) => index,
            None => {
                let index = nearest(key, palette);
                if let Some(free) = free {
                    cache[free] = Some((key, index as u8));
                }
                index
            }
        };
        sample[..3].copy_from_slice(&palette[index]);
    }
    output
}

fn median(mut samples: Vec<u128>) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}
fn sample(mut f: impl FnMut(), reps: usize) -> u128 {
    let start = Instant::now();
    for _ in 0..reps {
        f();
    }
    start.elapsed().as_nanos() / reps as u128
}

fn fixture(width: u32, height: u32, colors: usize, varied: bool) -> (Vec<Pixel>, Vec<[u8; 3]>) {
    let palette: Vec<[u8; 3]> = (0..colors)
        .map(|i| {
            let k = (i as u32).wrapping_mul(0x9E37_79B9);
            [(k >> 16) as u8, (k >> 8) as u8, k as u8]
        })
        .collect();
    let input = (0..width * height)
        .map(|i| {
            let k = i.wrapping_mul(0x7FEB_352D).rotate_left(9);
            let rgb = if varied {
                [(k >> 16) as u8, (k >> 8) as u8, k as u8]
            } else {
                palette[i as usize % colors]
            };
            [rgb[0], rgb[1], rgb[2], if i % 7 == 0 { 0 } else { 200 }]
        })
        .collect();
    (input, palette)
}

fn run_case(width: u32, height: u32, colors: usize, varied: bool, reps: usize) {
    let (input, palette) = fixture(width, height, colors, varied);
    let expected = direct(&input, &palette);
    let alternative = cached(&input, &palette);
    assert_eq!(expected, alternative, "cache changed exact RGBA");
    let source = Image::from_pixels(
        width,
        height,
        &input,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let mut production = source.try_copy(&mut Work::default()).unwrap();
    let units = input.len() as u64 * colors as u64;
    let mut budget = Work::new(units).unwrap();
    production
        .quantize_nearest(&palette, None, &mut budget)
        .unwrap();
    assert_eq!(production.pixels(), expected);
    assert_eq!(budget.remaining(), 0);

    let result = Image::from_pixels(
        width,
        height,
        &alternative,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let mut png_a = Vec::new();
    let mut png_b = Vec::new();
    png::encode_compressed(&production, &mut png_a, &mut Work::default()).unwrap();
    png::encode_compressed(&result, &mut png_b, &mut Work::default()).unwrap();
    assert_eq!(png_a, png_b, "PNG output diverged");

    let mut a = Vec::new();
    let mut b = Vec::new();
    for i in 0..7 {
        if i % 2 == 0 {
            a.push(sample(
                || {
                    black_box(direct(black_box(&input), black_box(&palette)));
                },
                reps,
            ));
            b.push(sample(
                || {
                    black_box(cached(black_box(&input), black_box(&palette)));
                },
                reps,
            ));
        } else {
            b.push(sample(
                || {
                    black_box(cached(black_box(&input), black_box(&palette)));
                },
                reps,
            ));
            a.push(sample(
                || {
                    black_box(direct(black_box(&input), black_box(&palette)));
                },
                reps,
            ));
        }
    }
    let (a, b) = (median(a), median(b));
    println!(
        "width={width} height={height} palette={colors} data={} direct_ns={a} cached_ns={b} cached/direct={:.3} png_bytes={} identical_rgba=true identical_png=true equal_work=true scratch_direct_bytes=0 scratch_cached_max_bytes={} memory=logical_not_rss",
        if varied { "varied" } else { "repeated" },
        b as f64 / a as f64,
        png_a.len(),
        std::mem::size_of::<[Option<([u8; 3], u8)>; CACHE_SLOTS]>()
    );
}

fn main() {
    println!(
        "benchmark=quantize platform={}-{} rust=1.99.1 profile=release rounds=7",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    run_case(16, 16, 8, false, 250);
    run_case(64, 64, 16, false, 80);
    run_case(64, 64, 16, true, 80);
    run_case(256, 256, 64, false, 12);
    run_case(256, 256, 64, true, 12);
    run_case(1024, 1024, 16, false, 2);
    run_case(1024, 1024, 16, true, 2);
}
