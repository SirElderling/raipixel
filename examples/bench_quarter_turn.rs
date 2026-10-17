//! Two exact quarter-turn strategies: destination-row gather vs source-row scatter.
//! cargo +1.99.1 run --release --locked --offline --example bench_quarter_turn
//! Neither candidate is compiled into the production library as a second backend.
use raipixel::{Image, Limits, Pixel, Transform, Work, png};
use std::{hint::black_box, time::Instant};

type Rotation = fn(&[Pixel], usize, usize, bool) -> Vec<Pixel>;

fn gather(src: &[Pixel], width: usize, height: usize, clockwise: bool) -> Vec<Pixel> {
    let mut result = vec![[0; 4]; src.len()];
    for y in 0..width {
        for x in 0..height {
            let source_x = if clockwise { y } else { width - 1 - y };
            let source_y = if clockwise { height - 1 - x } else { x };
            result[y * height + x] = src[source_y * width + source_x];
        }
    }
    result
}

fn scatter(src: &[Pixel], width: usize, height: usize, clockwise: bool) -> Vec<Pixel> {
    let mut result = vec![[0; 4]; src.len()];
    for y in 0..height {
        for x in 0..width {
            let (destination_x, destination_y) = if clockwise {
                (height - 1 - y, x)
            } else {
                (y, width - 1 - x)
            };
            result[destination_y * height + destination_x] = src[y * width + x];
        }
    }
    result
}

fn source(width: usize, height: usize) -> Vec<Pixel> {
    (0..width * height)
        .map(|i| {
            let x = i % width;
            let y = i / width;
            [
                (x.wrapping_mul(7) ^ y.wrapping_mul(23)) as u8,
                (x.wrapping_mul(11) + y.wrapping_mul(5)) as u8,
                (i as u32 % 67) as u8,
                (i as u32 % 9 * 31) as u8,
            ]
        })
        .collect()
}

fn full(
    src: &[Pixel],
    width: usize,
    height: usize,
    clockwise: bool,
    method: Rotation,
) -> (usize, u8) {
    let pixels = method(black_box(src), width, height, clockwise);
    let checksum = pixels[0][0];
    let image = Image::from_pixels(
        height as u32,
        width as u32,
        &pixels,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let mut encoded = Vec::new();
    png::encode_compressed(&image, &mut encoded, &mut Work::default()).unwrap();
    (black_box(encoded.len()), checksum)
}

fn measure(
    src: &[Pixel],
    width: usize,
    height: usize,
    clockwise: bool,
    method: Rotation,
    repetitions: usize,
    with_png: bool,
) -> f64 {
    let start = Instant::now();
    for _ in 0..repetitions {
        if with_png {
            black_box(full(src, width, height, clockwise, method));
        } else {
            black_box(method(black_box(src), width, height, clockwise));
        }
    }
    start.elapsed().as_secs_f64() * 1e6 / repetitions as f64
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn main() {
    println!("quarter-turn A/B: release, alternate order; microseconds per complete call");
    println!("A=gather destination rows; B=scatter source rows; both allocate one RGBA Vec");
    println!("full includes Vec, Image::from_pixels, png::encode_compressed, no filesystem I/O");
    for (width, height, repeats, full_repeats) in [
        (16, 16, 1000, 60),
        (64, 64, 160, 20),
        (256, 256, 24, 5),
        (1024, 256, 6, 2),
        (1024, 1024, 2, 1),
    ] {
        let samples = source(width, height);
        for clockwise in [true, false] {
            let a = gather(&samples, width, height, clockwise);
            let b = scatter(&samples, width, height, clockwise);
            assert_eq!(a, b);
            let mut actual = Image::from_pixels(
                width as u32,
                height as u32,
                &samples,
                Limits::default(),
                &mut Work::default(),
            )
            .unwrap();
            actual
                .transform(
                    if clockwise {
                        Transform::Rotate90
                    } else {
                        Transform::Rotate270
                    },
                    &mut Work::default(),
                )
                .unwrap();
            assert_eq!(actual.size(), [height as u32, width as u32]);
            assert_eq!(actual.pixels(), a);
            // Both algorithms must produce exactly the same encoded PNG bytes.
            let mut left = Vec::new();
            let mut right = Vec::new();
            let one = Image::from_pixels(
                height as u32,
                width as u32,
                &a,
                Limits::default(),
                &mut Work::default(),
            )
            .unwrap();
            let two = Image::from_pixels(
                height as u32,
                width as u32,
                &b,
                Limits::default(),
                &mut Work::default(),
            )
            .unwrap();
            png::encode_compressed(&one, &mut left, &mut Work::default()).unwrap();
            png::encode_compressed(&two, &mut right, &mut Work::default()).unwrap();
            assert_eq!(left, right);
            let mut rotate_a = Vec::new();
            let mut rotate_b = Vec::new();
            let mut complete_a = Vec::new();
            let mut complete_b = Vec::new();
            for round in 0..7 {
                let order: [Rotation; 2] = if round % 2 == 0 {
                    [gather, scatter]
                } else {
                    [scatter, gather]
                };
                for method in order {
                    let value = measure(&samples, width, height, clockwise, method, repeats, false);
                    if std::ptr::fn_addr_eq(method, gather as Rotation) {
                        rotate_a.push(value);
                    } else {
                        rotate_b.push(value);
                    }
                }
                for method in order {
                    let value = measure(
                        &samples,
                        width,
                        height,
                        clockwise,
                        method,
                        full_repeats,
                        true,
                    );
                    if std::ptr::fn_addr_eq(method, gather as Rotation) {
                        complete_a.push(value);
                    } else {
                        complete_b.push(value);
                    }
                }
            }
            let (ka, kb, fa, fb) = (
                median(rotate_a),
                median(rotate_b),
                median(complete_a),
                median(complete_b),
            );
            println!(
                "{}x{} {} pixels: kernel A={:.3} B={:.3} B/A={:.3}; full A={:.3} B={:.3} B/A={:.3}; PNG={} bytes",
                width,
                height,
                if clockwise { "CW" } else { "CCW" },
                ka,
                kb,
                kb / ka,
                fa,
                fb,
                fb / fa,
                left.len()
            );
        }
    }
}
