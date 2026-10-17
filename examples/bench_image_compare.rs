//! A/B: full exact RGBA revision comparison versus equality-skipping rows.
//! A is a frozen full-scan reference; B uses the selected public Image::compare.
//! Keep the algorithms separate to avoid benchmarking the selected method twice.
//! cargo run --release --locked --offline --example bench_image_compare
use raipixel::{Error, Image, Limits, Rect, Work, compare::ImageDiff};
use std::{hint::black_box, time::Instant};

fn full_scan(
    left: &Image,
    right: &Image,
    region: Option<[u32; 4]>,
    work: &mut Work,
) -> Result<ImageDiff, Error> {
    if left.size() != right.size() {
        return Err(Error::Dimensions);
    }
    let [w, h] = left.size();
    let [x, y, rw, rh] = region.unwrap_or([0, 0, w, h]);
    if rw == 0
        || rh == 0
        || u64::from(x) + u64::from(rw) > u64::from(w)
        || u64::from(y) + u64::from(rh) > u64::from(h)
    {
        return Err(Error::Bounds);
    }
    work.charge(u64::from(rw) * u64::from(rh))?;
    let mut changed_samples = 0u64;
    let mut changed_visible_colors = 0u64;
    let mut changed_alpha = 0u64;
    let (mut min_x, mut min_y) = (x + rw, y + rh);
    let (mut max_x, mut max_y) = (x, y);
    for yy in y..y + rh {
        let offset = (yy as usize * w as usize) + x as usize;
        let a = &left.pixels()[offset..offset + rw as usize];
        let b = &right.pixels()[offset..offset + rw as usize];
        for (xx, (&old, &new)) in a.iter().zip(b).enumerate() {
            if old == new {
                continue;
            }
            changed_samples += 1;
            changed_alpha += u64::from(old[3] != new[3]);
            changed_visible_colors += u64::from(
                (old[3] == 0) != (new[3] == 0)
                    || ((old[3] != 0 || new[3] != 0) && old[..3] != new[..3]),
            );
            let px = x + xx as u32;
            min_x = min_x.min(px);
            min_y = min_y.min(yy);
            max_x = max_x.max(px);
            max_y = max_y.max(yy);
        }
    }
    Ok(ImageDiff {
        region: [x, y, rw, rh],
        changed_samples,
        changed_visible_colors,
        changed_alpha,
        changed_bbox: (changed_samples != 0).then_some([
            min_x,
            min_y,
            max_x - min_x + 1,
            max_y - min_y + 1,
        ]),
    })
}

fn fixture(side: u32, changed: usize, hidden_only: bool) -> (Image, Image) {
    let count = (side as usize) * (side as usize);
    let mut first = Vec::with_capacity(count);
    let mut second = Vec::with_capacity(count);
    for i in 0..count {
        let seed = (i as u32).wrapping_mul(0x9E37_79B9).rotate_left(11);
        let color = [
            (seed & 255) as u8,
            ((seed >> 8) & 255) as u8,
            ((seed >> 16) & 255) as u8,
            if i % 11 == 0 { 0 } else { 255 },
        ];
        let mut after = color;
        if changed != 0 && i % changed == 0 {
            if hidden_only {
                if color[3] == 0 {
                    after[0] ^= 0xFF;
                }
            } else {
                after[0] ^= 0xFF;
                if i % 3 == 0 {
                    after[3] ^= 0xFF;
                }
            }
        }
        first.push(color);
        second.push(after);
    }
    (
        Image::from_pixels(side, side, &first, Limits::default(), &mut Work::default()).unwrap(),
        Image::from_pixels(side, side, &second, Limits::default(), &mut Work::default()).unwrap(),
    )
}

fn nanos_per_iteration(mut f: impl FnMut(), iterations: usize) -> u128 {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed().as_nanos() / iterations as u128
}
fn median(mut observations: Vec<u128>) -> u128 {
    observations.sort_unstable();
    observations[observations.len() / 2]
}
fn case(name: &str, side: u32, changed: usize, hidden_only: bool, partial: bool) {
    let (left, right) = fixture(side, changed, hidden_only);
    let region = if partial {
        Some([side / 4, side / 4, side / 2, side / 2])
    } else {
        None
    };
    let rect = region.map(|[x, y, w, h]| Rect::new(x as i32, y as i32, w, h).unwrap());
    let mut a_work = Work::default();
    let mut b_work = Work::default();
    let a_result = full_scan(&left, &right, region, &mut a_work);
    let b_result = left.compare(&right, rect, &mut b_work);
    assert_eq!(a_result, b_result, "{name}: exact result");
    assert_eq!(
        a_work.remaining(),
        b_work.remaining(),
        "{name}: charge parity"
    );
    let iterations = match side {
        0..=16 => 7000,
        17..=64 => 1000,
        65..=256 => 100,
        _ => 10,
    };
    let mut a_times = Vec::new();
    let mut b_times = Vec::new();
    for round in 0..7 {
        let a = || {
            black_box(full_scan(&left, &right, region, &mut Work::default()).unwrap());
        };
        let b = || {
            black_box(left.compare(&right, rect, &mut Work::default()).unwrap());
        };
        if round % 2 == 0 {
            a_times.push(nanos_per_iteration(a, iterations));
            b_times.push(nanos_per_iteration(b, iterations));
        } else {
            b_times.push(nanos_per_iteration(b, iterations));
            a_times.push(nanos_per_iteration(a, iterations));
        }
    }
    let a = median(a_times);
    let b = median(b_times);
    println!(
        "case={name} size={side} changed={} a_scan_ns={a} b_row_skip_ns={b}          b_over_a={:.3} temporary_heap_bytes_A=0 temporary_heap_bytes_B=0          output_equal=true work_equal=true",
        a_result.unwrap().changed_samples,
        b as f64 / a as f64,
    );
}

fn main() {
    println!(
        "benchmark=exact_image_diff platform={}-{} profile=release rounds=7          A=one_pass B=skip_equal_rows build={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        env!("CARGO_PKG_VERSION"),
    );
    case("tiny_identical", 16, 0, false, false);
    case("tiny_single", 16, 257, false, false);
    case("icon_sparse", 64, 4093, false, false);
    case("icon_dense", 64, 2, false, false);
    case("sheet_identical", 1024, 0, false, false);
    case("sheet_sparse", 1024, 8191, false, false);
    case("sheet_dense", 1024, 2, false, false);
    case("sheet_hidden_rgb", 1024, 101, true, false);
    case("sheet_region", 1024, 257, false, true);
    for changed in [0usize, 1, 2, 3, 7, 31, 97] {
        for size in [16, 32, 64, 128] {
            let (left, right) = fixture(size, changed, changed % 2 == 1);
            for region in [None, Some([size / 4, size / 4, size / 2, size / 2])] {
                let rect = region.map(|[x, y, w, h]| Rect::new(x as i32, y as i32, w, h).unwrap());
                assert_eq!(
                    full_scan(&left, &right, region, &mut Work::default()),
                    left.compare(&right, rect, &mut Work::default()),
                    "adversarial {size} / {changed}",
                );
            }
        }
    }
    println!("adversarial_parity=pass");
}
