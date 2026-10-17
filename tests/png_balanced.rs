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
