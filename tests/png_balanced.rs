#![cfg(feature = "experimental-quality")]
use raipixel::{Image, Limits, Work, png};

fn patterned_image(width: usize, height: usize, palette: usize) -> Image {
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let index = ((x / 7) + (y / 5)) % palette;
            let v = index as u8;
            pixels.push([
                v,
                v.wrapping_mul(37),
                v.wrapping_mul(83),
                [0, 64, 128, 255][index % 4],
            ]);
        }
    }
    Image::from_pixels(
        width as u32,
        height as u32,
        &pixels,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn balanced_encoder_exact_hidden_rgba_and_deterministic() {
    let image = patterned_image(257, 129, 16);
    let mut data = Vec::new();
    assert!(png::encode_indexed_balanced(&image, &mut data, &mut Work::default()).unwrap());
    assert_eq!(
        png::decode(&data, Limits::default(), &mut Work::default())
            .unwrap()
            .image,
        image
    );
    let mut repeat = Vec::new();
    assert!(png::encode_indexed_balanced(&image, &mut repeat, &mut Work::default()).unwrap());
    assert_eq!(data, repeat);
    assert!(
        data.len() < 1000,
        "balanced diagonal art unexpectedly large"
    );
}

#[test]
fn balanced_work_exhaustion_is_transactional() {
    let image = patterned_image(64, 64, 5);
    let mut output = vec![0xa5];
    let err =
        png::encode_indexed_balanced(&image, &mut output, &mut Work::new(1).unwrap()).unwrap_err();
    assert_eq!(err.code(), "E_LIMIT");
    assert_eq!(output, [0xa5]);

    let mut successes = Vec::new();
    let mut work = Work::default();
    assert!(png::encode_indexed_balanced(&image, &mut successes, &mut work).unwrap());
    let spent = 1_000_000_000 - work.remaining();
    let mut insufficient = vec![0x5d];
    let err = png::encode_indexed_balanced(
        &image,
        &mut insufficient,
        &mut Work::new(spent - 1).unwrap(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "E_LIMIT");
    assert_eq!(insufficient, [0x5d]);
    let mut exact = Vec::new();
    let mut work = Work::new(spent).unwrap();
    assert!(png::encode_indexed_balanced(&image, &mut exact, &mut work).unwrap());
    assert_eq!(work.remaining(), 0);
    assert_eq!(exact, successes);
}

#[test]
fn balanced_unsupported_palette_and_caller_io_failure() {
    let pixels: Vec<_> = (0..257u16)
        .map(|x| [x as u8, (x >> 8) as u8, 99, 0])
        .collect();
    let unsupported =
        Image::from_pixels(257, 1, &pixels, Limits::default(), &mut Work::default()).unwrap();
    let mut output = vec![0xc3];
    assert!(
        !png::encode_indexed_balanced(&unsupported, &mut output, &mut Work::default()).unwrap()
    );
    assert_eq!(output, [0xc3]);

    struct Reject {
        calls: usize,
    }
    impl std::io::Write for Reject {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.calls += 1;
            Err(std::io::Error::other("test commit rejected"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let image = patterned_image(64, 64, 5);
    let mut reject = Reject { calls: 0 };
    let err = png::encode_indexed_balanced(&image, &mut reject, &mut Work::default()).unwrap_err();
    assert_eq!(err.code(), "E_IO");
    assert_eq!(reject.calls, 1);
}

#[test]
fn balanced_png_preserves_c24_independent_art_champions() {
    // Exact RGBA of the C16/C24 independent seeded art fixtures. Their
    // diagonal and banded indexes do not depend on the fixture PRNG.
    for (width, height, seed, diagonal, expected) in [
        (257usize, 129usize, 4u8, true, 517usize),
        (512, 128, 5u8, false, 445),
    ] {
        let palette: Vec<[u8; 4]> = (0..16u8)
            .map(|i| {
                [
                    i,
                    i.wrapping_mul(37).wrapping_add(seed.wrapping_mul(3)),
                    i.wrapping_mul(83).wrapping_add(seed.wrapping_mul(7)),
                    [0, 64, 128, 255][usize::from(i.wrapping_add(seed) % 4)],
                ]
            })
            .collect();
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let index = if diagonal {
                    ((x + y) / 7) % 16
                } else {
                    ((y / 4) + (x / 73)) % 16
                };
                pixels.push(palette[index]);
            }
        }
        let image = Image::from_pixels(
            width as u32,
            height as u32,
            &pixels,
            Limits::default(),
            &mut Work::default(),
        )
        .unwrap();
        let mut encoded = Vec::new();
        assert!(png::encode_indexed_balanced(&image, &mut encoded, &mut Work::default()).unwrap());
        assert_eq!(encoded.len(), expected, "C24 known size winner regressed");
        assert_eq!(
            png::decode(&encoded, Limits::default(), &mut Work::default())
                .unwrap()
                .image,
            image,
            "lossless alpha-zero RGBA changed"
        );
    }
}

#[test]
fn balanced_handles_maximum_side_and_odd_indexed_rows() {
    // Exact-RGBA profiles: one-row maximum width, one-column maximum height,
    // and odd widths spanning all indexed bit depths (1, 2, 4, 8).
    for (width, height, colors) in [
        (4096usize, 1usize, 4usize),
        (1, 4096, 5),
        (257, 7, 256),
        (13, 11, 3),
        (17, 11, 5),
        (7, 11, 1),
    ] {
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let i = (x + y * 13) % colors;
                let value = i as u8;
                pixels.push([
                    value,
                    value.wrapping_mul(37),
                    value.wrapping_mul(83),
                    [0, 64, 128, 255][i % 4],
                ]);
            }
        }
        let image = Image::from_pixels(
            width as u32,
            height as u32,
            &pixels,
            Limits::default(),
            &mut Work::default(),
        )
        .unwrap();
        let mut encoded = Vec::new();
        assert!(png::encode_indexed_balanced(&image, &mut encoded, &mut Work::default()).unwrap());
        let restored = png::decode(&encoded, Limits::default(), &mut Work::default())
            .unwrap()
            .image;
        assert_eq!(
            restored, image,
            "indexed roundtrip failed at {width}x{height}"
        );
        let mut second = Vec::new();
        assert!(png::encode_indexed_balanced(&image, &mut second, &mut Work::default()).unwrap());
        assert_eq!(second, encoded, "non-deterministic PNG at {width}x{height}");
    }
}

#[test]
fn balanced_rejects_max_width_memory_shortfall_before_work() {
    let pixels = vec![[11, 29, 41, 0]; 4096];
    // The RGBA input fits in 64 KiB. The 3-mode encoder workspace does not.
    let bounds = Limits::new(4096, 4096, 64 * 1024).unwrap();
    let image = Image::from_pixels(4096, 1, &pixels, bounds, &mut Work::default()).unwrap();
    let mut out = vec![0x5d, 0xfa];
    let mut work = Work::default();
    let before = work.remaining();
    let err = png::encode_indexed_balanced(&image, &mut out, &mut work).unwrap_err();
    assert_eq!(err.code(), "E_LIMIT");
    assert_eq!(out, [0x5d, 0xfa]);
    assert_eq!(work.remaining(), before);
}

#[test]
fn balanced_outputs_are_crc_protected() {
    let image = patterned_image(13, 11, 3);
    let mut output = Vec::new();
    assert!(png::encode_indexed_balanced(&image, &mut output, &mut Work::default()).unwrap());
    // Signature (8) + IHDR (25) + PLTE chunk header (8) = 41.
    assert_eq!(&output[37..41], b"PLTE");
    let mut corrupted = output;
    corrupted[41] ^= 1;
    let err = png::decode(&corrupted, Limits::default(), &mut Work::default()).unwrap_err();
    assert_eq!(err.code(), "E_PNG_CRC");
}
