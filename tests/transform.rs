use raipixel::{Error, Image, Limits, Pixel, Resource, Transform, Work};

const A: Pixel = [11, 12, 13, 0];
const B: Pixel = [21, 22, 23, 255];
const C: Pixel = [31, 32, 33, 127];
const D: Pixel = [41, 42, 43, 0];
const E: Pixel = [51, 52, 53, 1];
const F: Pixel = [61, 62, 63, 254];

fn image(samples: &[Pixel], width: u32, height: u32) -> Image {
    Image::from_pixels(
        width,
        height,
        samples,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn all_three_permutations_preserve_exact_rgba_and_dimensions() {
    let original = [A, B, C, D, E, F];
    for (kind, expected) in [
        (Transform::FlipX, [C, B, A, F, E, D]),
        (Transform::FlipY, [D, E, F, A, B, C]),
        (Transform::Rotate180, [F, E, D, C, B, A]),
    ] {
        let mut canvas = image(&original, 3, 2);
        let mut budget = Work::new(6).unwrap();
        assert_eq!(canvas.transform(kind, &mut budget), Ok(()));
        assert_eq!(canvas.size(), [3, 2]);
        assert_eq!(canvas.pixels(), &expected);
        assert_eq!(budget.remaining(), 0);
    }
}

#[test]
fn transformations_are_involutions_and_composition_equals_rotation() {
    for (width, height) in [(1, 1), (1, 7), (6, 1), (2, 3), (5, 4)] {
        let samples: Vec<_> = (0..width * height)
            .map(|n| [(n as u8).wrapping_mul(13), n as u8, 201, (n % 4) as u8])
            .collect();
        let original = image(&samples, width, height);
        for kind in [Transform::FlipX, Transform::FlipY, Transform::Rotate180] {
            let mut canvas = original.try_copy(&mut Work::default()).unwrap();
            canvas.transform(kind, &mut Work::default()).unwrap();
            canvas.transform(kind, &mut Work::default()).unwrap();
            assert_eq!(canvas, original, "{kind:?} {width}x{height}");
        }
        let mut flips = original.try_copy(&mut Work::default()).unwrap();
        flips
            .transform(Transform::FlipX, &mut Work::default())
            .unwrap();
        flips
            .transform(Transform::FlipY, &mut Work::default())
            .unwrap();
        let mut rotated = original.try_copy(&mut Work::default()).unwrap();
        rotated
            .transform(Transform::Rotate180, &mut Work::default())
            .unwrap();
        assert_eq!(flips, rotated);
    }
}

#[test]
fn budget_failures_are_transactional_and_not_charged() {
    let mut canvas = image(&[A, B, C, D, E, F], 3, 2);
    let mut work = Work::new(5).unwrap();
    let before = canvas.try_copy(&mut Work::default()).unwrap();
    assert_eq!(
        canvas.transform(Transform::FlipY, &mut work),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(work.remaining(), 5);
    assert_eq!(canvas, before);
}

#[test]
fn quarter_turns_match_independent_rectangular_vectors() {
    let original = [A, B, C, D, E, F];
    for (kind, expected) in [
        (Transform::Rotate90, [D, A, E, B, F, C]),
        (Transform::Rotate270, [C, F, B, E, A, D]),
    ] {
        let mut canvas = image(&original, 3, 2);
        let mut budget = Work::new(6).unwrap();
        assert_eq!(canvas.transform(kind, &mut budget), Ok(()));
        assert_eq!(canvas.size(), [2, 3]);
        assert_eq!(canvas.pixels(), &expected);
        assert_eq!(budget.remaining(), 0);
    }
}

#[test]
fn quarter_turns_are_exact_inverses_on_all_small_rectangles() {
    for width in 1..=8 {
        for height in 1..=9 {
            let original: Vec<_> = (0..width * height)
                .map(|n| [n as u8, (n * 13) as u8, (n * 29) as u8, (n % 5) as u8])
                .collect();
            let source = image(&original, width, height);
            for (first, second) in [
                (Transform::Rotate90, Transform::Rotate270),
                (Transform::Rotate270, Transform::Rotate90),
            ] {
                let mut canvas = source.try_copy(&mut Work::default()).unwrap();
                canvas.transform(first, &mut Work::default()).unwrap();
                canvas.transform(second, &mut Work::default()).unwrap();
                assert_eq!(canvas, source, "{width} x {height}, {first:?}");
            }
            let mut canvas = source.try_copy(&mut Work::default()).unwrap();
            for _ in 0..4 {
                canvas.transform(Transform::Rotate90, &mut Work::default()).unwrap();
            }
            assert_eq!(canvas, source);
        }
    }
}

#[test]
fn quarter_turn_rejects_budget_without_altering_cel() {
    let mut canvas = image(&[A, B, C, D, E, F], 3, 2);
    let original = canvas.try_copy(&mut Work::default()).unwrap();
    let mut small = Work::new(5).unwrap();
    assert_eq!(
        canvas.transform(Transform::Rotate90, &mut small),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(small.remaining(), 5);
    assert_eq!(canvas, original);

    let limits = Limits::new(3, 6, 24).unwrap();
    let mut limited = Image::from_pixels(3, 2, &[A, B, C, D, E, F], limits, &mut Work::default()).unwrap();
    let mut enough = Work::new(6).unwrap();
    assert_eq!(limited.transform(Transform::Rotate270, &mut enough), Ok(()));
    assert_eq!(enough.remaining(), 0);
    assert_eq!(limited.size(), [2, 3]);
}
