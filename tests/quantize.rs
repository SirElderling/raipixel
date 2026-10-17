use raipixel::{Error, Image, Limits, Resource, Work, Rect};

fn image(width: u32, height: u32, samples: &[[u8; 4]]) -> Image {
    Image::from_pixels(width, height, samples, Limits::default(), &mut Work::default()).unwrap()
}

#[test]
fn quantize_chooses_earlier_palette_entry_on_ties_and_keeps_alpha() {
    let samples = [
        [5, 0, 0, 255],
        [6, 0, 0, 128],
        [2, 3, 4, 0],
        [7, 0, 0, 1],
        [99, 99, 99, 0],
    ];
    let palette = [[0, 0, 0], [10, 0, 0]];
    let mut actual = image(5, 1, &samples);
    let mut work = Work::new(10).unwrap();
    assert_eq!(actual.quantize_nearest(&palette, None, &mut work), Ok(3));
    assert_eq!(
        actual.pixels(),
        &[
            [0, 0, 0, 255],
            [10, 0, 0, 128],
            [2, 3, 4, 0],
            [10, 0, 0, 1],
            [99, 99, 99, 0],
        ]
    );
    assert_eq!(work.remaining(), 0);
}

#[test]
fn quantize_uses_squared_rgb_distance_and_ignores_hidden_rgb() {
    let palette = [[2, 12, 18], [20, 3, 10], [8, 2, 10]];
    let mut canvas = image(2, 2, &[
        [0, 10, 20, 255],
        [20, 4, 11, 255],
        [2, 3, 4, 0],
        [8, 2, 11, 191],
    ]);
    assert_eq!(canvas.quantize_nearest(&palette, None, &mut Work::default()), Ok(3));
    assert_eq!(
        canvas.pixels(),
        &[
            [2, 12, 18, 255],
            [20, 3, 10, 255],
            [2, 3, 4, 0],
            [8, 2, 10, 191],
        ]
    );
}

#[test]
fn quantize_region_and_rejected_work_are_transactional() {
    let samples = [[31, 6, 9, 255]; 9];
    let mut canvas = image(3, 3, &samples);
    let original = canvas.try_copy(&mut Work::default()).unwrap();
    let palette = [[0, 0, 0], [30, 0, 0]];
    let region = Rect::new(1, 1, 2, 1).unwrap();
    let mut budget = Work::new(4).unwrap();
    assert_eq!(canvas.quantize_nearest(&palette, Some(region), &mut budget), Ok(2));
    assert_eq!(budget.remaining(), 0);
    for y in 0..3 {
        for x in 0..3 {
            assert_eq!(canvas.pixel(x, y), Some(if y == 1 && x >= 1 {
                [30, 0, 0, 255]
            } else {
                samples[0]
            }));
        }
    }
    let snapshot = canvas.try_copy(&mut Work::default()).unwrap();
    let mut low = Work::new(2).unwrap();
    assert_eq!(canvas.quantize_nearest(&palette, None, &mut low), Err(Error::Limit(Resource::Work)));
    assert_eq!(low.remaining(), 2);
    assert_eq!(canvas, snapshot);
    let outside = Rect::new(-1, 0, 2, 2).unwrap();
    assert_eq!(canvas.quantize_nearest(&palette, Some(outside), &mut low), Err(Error::Bounds));
    assert_eq!(low.remaining(), 2);
    assert_ne!(canvas, original);
}

#[test]
fn quantize_rejects_invalid_palettes_before_modification() {
    let mut canvas = image(1, 1, &[[1, 2, 3, 255]]);
    let mut work = Work::new(1).unwrap();
    assert_eq!(canvas.quantize_nearest(&[], None, &mut work), Err(Error::PixelCount));
    assert_eq!(
        canvas.quantize_nearest(&[[0, 0, 0]; 257], None, &mut work),
        Err(Error::PixelCount)
    );
    assert_eq!(canvas.pixel(0, 0), Some([1, 2, 3, 255]));
    assert_eq!(work.remaining(), 1);
}
