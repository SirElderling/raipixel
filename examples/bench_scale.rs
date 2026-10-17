//! Compare prior two-buffer nearest-neighbor export against one-buffer scaling.
//! cargo run --release --locked --offline --example bench_scale
use raipixel::{Error, Image, Limits, Resource, Work, png};
use std::{hint::black_box, time::Instant};

/// A: original CLI algorithm. Full-sized temporary Vec<Pixel>, followed
/// by a second Image allocation. This copy is benchmark-only after selection.
fn two_buffers(image: &Image, factor: u32, work: &mut Work) -> Result<Image, Error> {
    if factor == 0 {
        return Err(Error::Dimensions);
    }
    let [width, height] = image.size();
    let expanded_width = width
        .checked_mul(factor)
        .ok_or(Error::Limit(Resource::Side))?;
    let expanded_height = height
        .checked_mul(factor)
        .ok_or(Error::Limit(Resource::Side))?;
    if expanded_width > 4096 || expanded_height > 4096 {
        return Err(Error::Limit(Resource::Side));
    }
    let count = u64::from(expanded_width) * u64::from(expanded_height);
    if count > 16_777_216 {
        return Err(Error::Limit(Resource::Pixels));
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count as usize)
        .map_err(|_| Error::Allocation)?;
    for y in 0..expanded_height {
        for x in 0..expanded_width {
            pixels.push(image.pixel(x / factor, y / factor).unwrap());
        }
    }
    Image::from_pixels(
        expanded_width,
        expanded_height,
        &pixels,
        Limits::default(),
        work,
    )
}

fn fixture(side: u32, noisy: bool) -> Image {
    let mut pixels = Vec::with_capacity((side * side) as usize);
    for i in 0..(side * side) {
        let k = if noisy {
            i.wrapping_mul(0x9E37_79B9).rotate_left(13)
        } else {
            ((i / side / 5) + (i % side / 5)) % 13
        };
        pixels.push([
            (k & 255) as u8,
            ((k >> 5) & 255) as u8,
            ((k >> 11) & 255) as u8,
            if i % 19 == 0 { 0 } else { 255 },
        ]);
    }
    Image::from_pixels(side, side, &pixels, Limits::default(), &mut Work::default()).unwrap()
}
fn benchmark(mut run: impl FnMut(), rounds: usize) -> u128 {
    let now = Instant::now();
    for _ in 0..rounds {
        run();
    }
    now.elapsed().as_nanos() / rounds as u128
}
fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}
fn encoded(image: &Image, mut work: Work) -> (Vec<u8>, u64) {
    let mut png_bytes = Vec::new();
    png::encode_compressed(image, &mut png_bytes, &mut work).unwrap();
    (png_bytes, work.remaining())
}
fn case(side: u32, factor: u32, noisy: bool, iterations: usize) {
    let src = fixture(side, noisy);
    let mut old_work = Work::default();
    let mut new_work = Work::default();
    let old = two_buffers(&src, factor, &mut old_work).unwrap();
    let new = src.scaled_nearest(factor, &mut new_work).unwrap();
    assert_eq!(old, new, "scaled RGBA differs");
    assert_eq!(
        old_work.remaining(),
        new_work.remaining(),
        "scaling work differs"
    );
    let (bytes_a, remain_a) = encoded(&old, old_work);
    let (bytes_b, remain_b) = encoded(&new, new_work);
    assert_eq!(bytes_a, bytes_b, "PNG output differs");
    assert_eq!(remain_a, remain_b, "full export work differs");

    let mut scale_a = Vec::new();
    let mut scale_b = Vec::new();
    let mut export_a = Vec::new();
    let mut export_b = Vec::new();
    for round in 0..7 {
        let mut old_scale = || {
            black_box(two_buffers(&src, factor, &mut Work::default()).unwrap());
        };
        let mut new_scale = || {
            black_box(src.scaled_nearest(factor, &mut Work::default()).unwrap());
        };
        let mut old_export = || {
            let scaled = two_buffers(&src, factor, &mut Work::default()).unwrap();
            black_box(encoded(&scaled, Work::default()).0);
        };
        let mut new_export = || {
            let scaled = src.scaled_nearest(factor, &mut Work::default()).unwrap();
            black_box(encoded(&scaled, Work::default()).0);
        };
        if round % 2 == 0 {
            scale_a.push(benchmark(&mut old_scale, iterations));
            scale_b.push(benchmark(&mut new_scale, iterations));
            export_a.push(benchmark(&mut old_export, iterations.min(5)));
            export_b.push(benchmark(&mut new_export, iterations.min(5)));
        } else {
            scale_b.push(benchmark(&mut new_scale, iterations));
            scale_a.push(benchmark(&mut old_scale, iterations));
            export_b.push(benchmark(&mut new_export, iterations.min(5)));
            export_a.push(benchmark(&mut old_export, iterations.min(5)));
        }
    }
    let sa = median(scale_a);
    let sb = median(scale_b);
    let ea = median(export_a);
    let eb = median(export_b);
    let pixels = side as usize * side as usize * factor as usize * factor as usize;
    println!(
        "side={side} factor={factor} noisy={noisy} bytes={}          old_scale_ns={sa} direct_scale_ns={sb} direct/old_scale={:.3}          old_full_export_ns={ea} direct_full_export_ns={eb} direct/old_full={:.3}          old_scaled_rgba_peak_logical_bytes={} direct_scaled_rgba_peak_logical_bytes={}          exact_pixels=true identical_png=true work_equal=true note=logical_not_RSS",
        bytes_a.len(),
        sb as f64 / sa as f64,
        eb as f64 / ea as f64,
        pixels * 8,
        pixels * 4,
    );
}

fn main() {
    println!(
        "benchmark=nearest_scale platform={}-{} rounds=7 profile=release",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    case(16, 8, false, 100);
    case(32, 8, false, 30);
    case(64, 4, false, 25);
    case(128, 4, false, 12);
    case(256, 4, false, 2);
    case(64, 4, true, 20);
    case(128, 4, true, 8);
}
