//! Two genuinely different exact transforms: in-place swaps vs out-of-place mapping.
//! cargo run --release --locked --offline --example bench_transform
use raipixel::{Error, Image, Limits, Pixel, Transform, Work, png};
use std::{hint::black_box, time::Instant};

fn fixture(width: u32, height: u32) -> Image {
    let samples: Vec<Pixel> = (0..width * height)
        .map(|i| {
            let k = i.wrapping_mul(0x9E37_79B9).rotate_left(7);
            [(k & 255) as u8, ((k >> 8) & 255) as u8, (k >> 16) as u8, if i % 7 == 0 { 0 } else { 255 }]
        })
        .collect();
    Image::from_pixels(width, height, &samples, Limits::default(), &mut Work::default()).unwrap()
}

/// B: build a new image by mapping every output position back to the input.
/// Uses only public safe Rust APIs. The temporary pixel array is intentional.
fn copy_map(input: &Image, kind: Transform, work: &mut Work) -> Result<Image, Error> {
    let [width, height] = input.size();
    let count = width as usize * height as usize;
    work.charge(count as u64)?;
    let mut samples = Vec::new();
    samples.try_reserve_exact(count).map_err(|_| Error::Allocation)?;
    for y in 0..height {
        for x in 0..width {
            let (sx, sy) = match kind {
                Transform::FlipX => (width - 1 - x, y),
                Transform::FlipY => (x, height - 1 - y),
                Transform::Rotate180 => (width - 1 - x, height - 1 - y),
            };
            samples.push(input.pixel(sx, sy).unwrap());
        }
    }
    // Internal image creation is part of measured CPU and allocations.
    // The equal external work charge represents the same output pixel visits.
    Image::from_pixels(width, height, &samples, Limits::default(), &mut Work::default())
}

fn round_trip(image: &Image) -> Vec<u8> {
    let mut encoded = Vec::new();
    png::encode_compressed(image, &mut encoded, &mut Work::default()).unwrap();
    encoded
}

fn timed(mut operation: impl FnMut(), reps: usize) -> u128 {
    let start = Instant::now();
    for _ in 0..reps {
        operation();
    }
    start.elapsed().as_nanos() / reps as u128
}

fn median(mut times: Vec<u128>) -> u128 {
    times.sort_unstable();
    times[times.len() / 2]
}

fn run_case(width: u32, height: u32, kind: Transform, iterations: usize) {
    let input = fixture(width, height);
    let mut a = input.try_copy(&mut Work::default()).unwrap();
    let mut wa = Work::default();
    a.transform(kind, &mut wa).unwrap();
    let mut wb = Work::default();
    let b = copy_map(&input, kind, &mut wb).unwrap();
    assert_eq!(a, b, "exact RGBA differs");
    assert_eq!(wa.remaining(), wb.remaining(), "caller work differs");
    let png_a = round_trip(&a);
    let png_b = round_trip(&b);
    assert_eq!(png_a, png_b, "PNG bytes differ");

    let mut a_times = Vec::new();
    let mut b_times = Vec::new();
    let mut a_exports = Vec::new();
    let mut b_exports = Vec::new();
    for n in 0..7 {
        let mut in_place = || {
            let mut scratch = input.try_copy(&mut Work::default()).unwrap();
            scratch.transform(kind, &mut Work::default()).unwrap();
            black_box(scratch);
        };
        let mut out_of_place = || {
            let scratch = input.try_copy(&mut Work::default()).unwrap();
            black_box(copy_map(&scratch, kind, &mut Work::default()).unwrap());
        };
        let mut export_a = || {
            let mut scratch = input.try_copy(&mut Work::default()).unwrap();
            scratch.transform(kind, &mut Work::default()).unwrap();
            black_box(round_trip(&scratch));
        };
        let mut export_b = || {
            let scratch = input.try_copy(&mut Work::default()).unwrap();
            let result = copy_map(&scratch, kind, &mut Work::default()).unwrap();
            black_box(round_trip(&result));
        };
        if n % 2 == 0 {
            a_times.push(timed(&mut in_place, iterations));
            b_times.push(timed(&mut out_of_place, iterations));
            a_exports.push(timed(&mut export_a, iterations.min(4)));
            b_exports.push(timed(&mut export_b, iterations.min(4)));
        } else {
            b_times.push(timed(&mut out_of_place, iterations));
            a_times.push(timed(&mut in_place, iterations));
            b_exports.push(timed(&mut export_b, iterations.min(4)));
            a_exports.push(timed(&mut export_a, iterations.min(4)));
        }
    }
    let ta = median(a_times);
    let tb = median(b_times);
    let ea = median(a_exports);
    let eb = median(b_exports);
    let bytes = width as usize * height as usize * 4;
    println!(
        "width={width} height={height} kind={kind:?} png_bytes={} in_place_ns={ta} copy_map_ns={tb} copy/in_place={:.3} full_in_place_ns={ea} full_copy_ns={eb} full_copy/in_place={:.3} extra_scratch_in_place_bytes=0 extra_scratch_copy_map_bytes={} identical_rgba=true identical_png=true equal_work=true memory=logical_not_rss",
        png_a.len(),
        tb as f64 / ta as f64,
        eb as f64 / ea as f64,
        bytes * 2
    );
}

fn main() {
    println!(
        "benchmark=transform platform={}-{} compiler=rust-1.99.1 profile=release rounds=7",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for kind in [Transform::FlipX, Transform::FlipY, Transform::Rotate180] {
        run_case(16, 16, kind, 120);
        run_case(64, 64, kind, 75);
        run_case(256, 256, kind, 18);
        run_case(1024, 1024, kind, 4);
    }
}
