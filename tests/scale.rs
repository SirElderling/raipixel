use raipixel::{Error, Image, Limits, Resource, Work};

fn fixture() -> Image {
    Image::from_pixels(
        3,
        2,
        &[
            [1, 2, 3, 0],
            [4, 5, 6, 128],
            [7, 8, 9, 255],
            [10, 11, 12, 1],
            [13, 14, 15, 254],
            [16, 17, 18, 0],
        ],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn integer_upscale_repeats_exact_samples_and_preserves_source() {
    let source = fixture();
    let before = source.try_copy(&mut Work::default()).unwrap();
    for factor in 1..=8 {
        let mut work = Work::new(6 * u64::from(factor * factor)).unwrap();
        let enlarged = source.scaled_nearest(factor, &mut work).unwrap();
        assert_eq!(enlarged.size(), [3 * factor, 2 * factor]);
        assert_eq!(work.remaining(), 0);
        for y in 0..(2 * factor) {
            for x in 0..(3 * factor) {
                assert_eq!(
                    enlarged.pixel(x, y),
                    source.pixel(x / factor, y / factor),
                    "factor={factor}, pixel=[{x},{y}]"
                );
            }
        }
    }
    assert_eq!(source, before);
}

#[test]
fn scale_rejects_zero_excessive_side_and_work_without_mutating() {
    let source = fixture();
    let before = source.try_copy(&mut Work::default()).unwrap();
    let mut budget = Work::new(10).unwrap();
    assert_eq!(source.scaled_nearest(0, &mut budget), Err(Error::Dimensions));
    assert_eq!(
        source.scaled_nearest(u32::MAX, &mut budget),
        Err(Error::Limit(Resource::Side))
    );
    assert_eq!(
        source.scaled_nearest(2, &mut budget),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(budget.remaining(), 10);
    assert_eq!(source, before);
}

#[test]
fn scale_respects_image_resource_ceiling_before_spending_work() {
    let limited = Image::from_pixels(
        2,
        2,
        &[[7, 8, 9, 0]; 4],
        Limits::new(3, 9, 48).unwrap(),
        &mut Work::default(),
    )
    .unwrap();
    let mut budget = Work::new(64).unwrap();
    assert_eq!(
        limited.scaled_nearest(2, &mut budget),
        Err(Error::Limit(Resource::Side))
    );
    assert_eq!(budget.remaining(), 64);
}
