//! Compare integer bounding-box tests (A) against analytical row spans (B).
//! Both operate through public image APIs, preserve hidden RGB, and emit equal PNGs.
//! Run: cargo run --release --locked --offline --example bench_ellipse
use raipixel::{Image, Limits, Pixel, Rect, Work, png};
use std::{hint::black_box, time::Instant};

const HIDDEN: Pixel = [13, 57, 101, 0];
const OUTLINE: Pixel = [197, 42, 81, 255];

fn fixture(width: u32, height: u32) -> Image {
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            pixels.push([
                x.wrapping_mul(37) as u8,
                y.wrapping_mul(11) as u8,
                (x ^ y).wrapping_mul(19) as u8,
                if (x + y) % 3 == 0 { 0 } else { 255 },
            ]);
        }
    }
    Image::from_pixels(
        width,
        height,
        &pixels,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

fn contains(x: i64, y: i64, geometry: [i64; 4]) -> bool {
    let [rx, ry, width, height] = geometry;
    if x < rx || y < ry || x >= rx + width || y >= ry + height {
        return false;
    }
    let xx = 2 * i128::from(x - rx) + 1 - i128::from(width);
    let yy = 2 * i128::from(y - ry) + 1 - i128::from(height);
    let w = i128::from(width);
    let h = i128::from(height);
    xx * xx * h * h + yy * yy * w * w <= w * w * h * h
}

/// B: binary-search the leftmost inside centre in each row, then mirror.
/// For perimeter-only rendering compare current span with its two neighbors.
/// This candidate deliberately uses public `fill` rather than private mutation,
/// so validation and per-span dispatch costs are included in measured timings.
fn span(y: i64, geometry: [i64; 4]) -> Option<(i64, i64)> {
    let [rx, ry, width, height] = geometry;
    if y < ry || y >= ry + height {
        return None;
    }
    let midpoint = rx + (width - 1) / 2;
    if !contains(midpoint, y, geometry) {
        return None;
    }
    let (mut left, mut right) = (rx, midpoint);
    while left < right {
        let candidate = left + (right - left) / 2;
        if contains(candidate, y, geometry) {
            right = candidate;
        } else {
            left = candidate + 1;
        }
    }
    Some((left, rx + width - 1 - left))
}

fn row_spans(canvas: &mut Image, rect: [i32; 4], color: Pixel, filled: bool, work: &mut Work) {
    let [rx, ry, w, h] = rect;
    let geometry = [i64::from(rx), i64::from(ry), i64::from(w), i64::from(h)];
    work.charge(w as u64 * h as u64).unwrap();
    let [width, height] = canvas.size();
    let width = i64::from(width);
    let height = i64::from(height);
    for y in (i64::from(ry)).max(0)..(i64::from(ry) + i64::from(h)).min(height) {
        let Some((lo, hi)) = span(y, geometry) else {
            continue;
        };
        let prev = span(y - 1, geometry);
        let next = span(y + 1, geometry);
        let (mut current, end) = (lo.max(0), (hi + 1).min(width));
        while current < end {
            let selected = filled
                || current == lo
                || current == hi
                || prev.is_none_or(|(a, b)| current < a || current > b)
                || next.is_none_or(|(a, b)| current < a || current > b);
            if !selected {
                current += 1;
                continue;
            }
            let start = current;
            current += 1;
            while current < end
                && (filled
                    || current == lo
                    || current == hi
                    || prev.is_none_or(|(a, b)| current < a || current > b)
                    || next.is_none_or(|(a, b)| current < a || current > b))
            {
                current += 1;
            }
            canvas
                .fill(
                    Rect::new(start as i32, y as i32, (current - start) as u32, 1).unwrap(),
                    color,
                    false,
                    &mut Work::default(),
                )
                .unwrap();
        }
    }
}

fn png_bytes(image: &Image) -> Vec<u8> {
    let mut output = Vec::new();
    png::encode_compressed(image, &mut output, &mut Work::default()).unwrap();
    output
}

fn timed(mut action: impl FnMut(), iterations: usize) -> u128 {
    let start = Instant::now();
    for _ in 0..iterations {
        action();
    }
    start.elapsed().as_nanos() / iterations as u128
}
fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn run_case(width: u32, height: u32, filled: bool, iterations: usize) {
    let source = fixture(width, height);
    let bounds = [0, 0, width as i32, height as i32];
    let shape = Rect::new(bounds[0], bounds[1], width, height).unwrap();
    let color = if filled { HIDDEN } else { OUTLINE };
    let mut a = source.try_copy(&mut Work::default()).unwrap();
    let mut b = source.try_copy(&mut Work::default()).unwrap();
    let mut wa = Work::default();
    let mut wb = Work::default();
    a.ellipse(shape, color, filled, false, &mut wa).unwrap();
    row_spans(&mut b, bounds, color, filled, &mut wb);
    assert_eq!(a, b, "raster differs {width}x{height} filled={filled}");
    assert_eq!(wa.remaining(), wb.remaining(), "charged work differs");
    let encoded = png_bytes(&a);
    assert_eq!(encoded, png_bytes(&b), "PNG differs");

    let mut a_times = Vec::new();
    let mut b_times = Vec::new();
    let mut a_full = Vec::new();
    let mut b_full = Vec::new();
    for round in 0..7 {
        let mut method_a = || {
            let mut working = source.try_copy(&mut Work::default()).unwrap();
            working
                .ellipse(shape, color, filled, false, &mut Work::default())
                .unwrap();
            black_box(working);
        };
        let mut method_b = || {
            let mut working = source.try_copy(&mut Work::default()).unwrap();
            row_spans(&mut working, bounds, color, filled, &mut Work::default());
            black_box(working);
        };
        let mut full_a = || {
            let mut working = source.try_copy(&mut Work::default()).unwrap();
            working
                .ellipse(shape, color, filled, false, &mut Work::default())
                .unwrap();
            black_box(png_bytes(&working));
        };
        let mut full_b = || {
            let mut working = source.try_copy(&mut Work::default()).unwrap();
            row_spans(&mut working, bounds, color, filled, &mut Work::default());
            black_box(png_bytes(&working));
        };
        if round % 2 == 0 {
            a_times.push(timed(&mut method_a, iterations));
            b_times.push(timed(&mut method_b, iterations));
            a_full.push(timed(&mut full_a, iterations.min(2)));
            b_full.push(timed(&mut full_b, iterations.min(2)));
        } else {
            b_times.push(timed(&mut method_b, iterations));
            a_times.push(timed(&mut method_a, iterations));
            b_full.push(timed(&mut full_b, iterations.min(2)));
            a_full.push(timed(&mut full_a, iterations.min(2)));
        }
    }
    let (a_ns, b_ns) = (median(a_times), median(b_times));
    let (a_full_ns, b_full_ns) = (median(a_full), median(b_full));
    println!(
        "size={width}x{height} filled={filled} png_bytes={} scan_ns={a_ns} span_ns={b_ns} span/scan={:.3} scan_export_ns={a_full_ns} span_export_ns={b_full_ns} span_export/scan_export={:.3} extra_scratch_bytes=0 peak_rss=unmeasured rgba=true png=true work=true",
        encoded.len(),
        b_ns as f64 / a_ns as f64,
        b_full_ns as f64 / a_full_ns as f64
    );
}

fn main() {
    println!(
        "benchmark=ellipse platform={}-{} compiler=rust-1.99.1 profile=release rounds=7",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for (w, h, iterations) in [
        (16, 16, 160),
        (32, 64, 90),
        (64, 64, 50),
        (256, 256, 12),
        (1024, 1024, 2),
    ] {
        for filled in [true, false] {
            run_case(w, h, filled, iterations);
        }
    }
}
