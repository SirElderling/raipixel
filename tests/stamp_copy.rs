//! Compare direct overlapping in-place copy against a separate immutable source.
use raipixel::{Blend, Error, Image, Limits, Rect, Work};

fn fixture(width: u32, height: u32) -> Image {
    let pixels: Vec<_> = (0..width * height)
        .map(|i| {
            let seed = i.wrapping_mul(0x9E37_79B9).rotate_left(11);
            [
                seed as u8,
                (seed >> 8) as u8,
                (seed >> 16) as u8,
                if i % 5 == 0 { 0 } else { (i % 255 + 1) as u8 },
            ]
        })
        .collect();
    Image::from_pixels(
        width,
        height,
        &pixels,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn exact_copy_matches_immutable_source_for_every_small_overlap_and_clip() {
    let original = fixture(5, 4);
    for x in 0..5 {
        for y in 0..4 {
            for width in 1..=5 - x {
                for height in 1..=4 - y {
                    let source = Rect::new(x as i32, y as i32, width, height).unwrap();
                    for dx in -5..=7 {
                        for dy in -4..=6 {
                            let mut actual = original.try_copy(&mut Work::default()).unwrap();
                            let mut expected = original.try_copy(&mut Work::default()).unwrap();
                            let mut a = Work::default();
                            let mut b = Work::default();
                            let got =
                                actual.stamp_within(source, [dx, dy], Blend::Copy, 255, true, &mut a);
                            let want = expected.stamp(
                                &original,
                                source,
                                [dx, dy],
                                Blend::Copy,
                                255,
                                true,
                                &mut b,
                            );
                            assert_eq!(got, want, "source {source:?}, target [{dx},{dy}]");
                            assert_eq!(
                                actual, expected,
                                "mismatch source {source:?}, target [{dx},{dy}]"
                            );
                            assert_eq!(
                                a.remaining(),
                                Work::default().remaining() - u64::from(width) * u64::from(height) * 2
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn copy_work_exhaustion_and_bad_bounds_leave_image_unchanged() {
    let mut img = fixture(4, 4);
    let original = img.try_copy(&mut Work::default()).unwrap();
    let rect = Rect::new(0, 0, 3, 3).unwrap();
    let mut budget = Work::new(17).unwrap();
    assert_eq!(
        img.stamp_within(rect, [1, 1], Blend::Copy, 255, false, &mut budget),
        Err(Error::Limit(raipixel::Resource::Work))
    );
    assert_eq!(budget.remaining(), 17);
    assert_eq!(img, original);
    assert_eq!(
        img.stamp_within(rect, [2, 2], Blend::Copy, 255, false, &mut budget),
        Err(Error::Bounds)
    );
    assert_eq!(budget.remaining(), 17);
    assert_eq!(img, original);
}

#[test]
fn partial_alpha_copy_keeps_exact_copy_opacity_semantics() {
    let original = fixture(4, 3);
    let mut img = original.try_copy(&mut Work::default()).unwrap();
    let rect = Rect::new(0, 0, 3, 2).unwrap();
    img.stamp_within(rect, [1, 1], Blend::Copy, 128, false, &mut Work::default())
        .unwrap();
    let mut expected = original.try_copy(&mut Work::default()).unwrap();
    expected
        .stamp(&original, rect, [1, 1], Blend::Copy, 128, false, &mut Work::default())
        .unwrap();
    assert_eq!(img, expected);
}
