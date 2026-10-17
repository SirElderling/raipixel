use raipixel::{Error, Image, Limits, Rect, Resource, Work};

fn image() -> Image {
    Image::from_pixels(
        3,
        2,
        &[
            [10, 20, 30, 0],
            [5, 6, 7, 128],
            [5, 6, 7, 128],
            [10, 20, 30, 0],
            [0, 1, 2, 255],
            [4, 3, 2, 0],
        ],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn inspect_counts_exact_transparent_rgb_and_nontransparent_bounds() {
    let image = image();
    let before = image.try_copy(&mut Work::default()).unwrap();
    let stats = image.inspect(None, 16, true, &mut Work::default()).unwrap();
    assert_eq!(stats.size, [3, 2]);
    assert_eq!(stats.region, [0, 0, 3, 2]);
    assert_eq!(stats.pixel_count, 6);
    assert_eq!(stats.zero_alpha, 3);
    assert_eq!(stats.partial_alpha, 2);
    assert_eq!(stats.opaque_alpha, 1);
    assert_eq!(stats.visible_bbox, Some([1, 0, 2, 2]));
    assert_eq!(stats.unique_colors, 4);
    assert!(!stats.histogram_truncated);
    assert_eq!(stats.histogram.len(), 4);
    assert_eq!(stats.histogram[0].rgba, [5, 6, 7, 128]);
    assert_eq!(stats.histogram[0].count, 2);
    assert_eq!(stats.histogram[1].rgba, [10, 20, 30, 0]);
    assert_eq!(stats.histogram[1].count, 2);
    assert_eq!(stats.pixels.as_deref(), Some(before.pixels()));
    assert_eq!(image, before);
}

#[test]
fn histogram_truncation_preserves_exact_unique_count_and_tie_order() {
    let image = image();
    let stats = image.inspect(None, 2, false, &mut Work::default()).unwrap();
    assert_eq!(stats.unique_colors, 4);
    assert!(stats.histogram_truncated);
    assert_eq!(stats.histogram.len(), 2);
    assert_eq!(stats.histogram[0].rgba, [5, 6, 7, 128]);
    assert_eq!(stats.histogram[1].rgba, [10, 20, 30, 0]);
    assert!(stats.pixels.is_none());
}

#[test]
fn inspect_region_is_image_local_and_does_not_include_other_samples() {
    let image = image();
    let stats = image
        .inspect(
            Some(Rect::new(0, 1, 3, 1).unwrap()),
            16,
            true,
            &mut Work::default(),
        )
        .unwrap();
    assert_eq!(stats.region, [0, 1, 3, 1]);
    assert_eq!(stats.pixel_count, 3);
    assert_eq!(stats.zero_alpha, 2);
    assert_eq!(stats.opaque_alpha, 1);
    assert_eq!(stats.partial_alpha, 0);
    assert_eq!(stats.visible_bbox, Some([1, 1, 1, 1]));
    assert_eq!(
        stats.pixels.unwrap(),
        [[10, 20, 30, 0], [0, 1, 2, 255], [4, 3, 2, 0]]
    );
}

#[test]
fn empty_visible_bbox_when_all_pixels_are_transparent_but_rgb_differs() {
    let image = Image::from_pixels(
        2,
        1,
        &[[1, 2, 3, 0], [5, 6, 7, 0]],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let stats = image.inspect(None, 16, false, &mut Work::default()).unwrap();
    assert_eq!(stats.visible_bbox, None);
    assert_eq!(stats.unique_colors, 2);
    assert_eq!(stats.zero_alpha, 2);
}

#[test]
fn invalid_region_histogram_bounds_and_raw_sample_limit_charge_no_work() {
    let image = image();
    let mut budget = Work::new(0).unwrap();
    assert_eq!(
        image.inspect(None, 0, false, &mut budget),
        Err(Error::PixelCount)
    );
    assert_eq!(
        image.inspect(
            Some(Rect::new(-1, 0, 1, 1).unwrap()),
            16,
            false,
            &mut budget,
        ),
        Err(Error::Bounds)
    );
    assert_eq!(budget.remaining(), 0);

    let big = Image::new(65, 65, Limits::default(), &mut Work::default()).unwrap();
    assert_eq!(
        big.inspect(None, 16, true, &mut Work::default()),
        Err(Error::Limit(Resource::Pixels))
    );
    assert_eq!(
        image.inspect(None, 16, false, &mut budget),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(budget.remaining(), 0);
}

#[test]
fn all_256_histogram_entries_are_supported() {
    let pixels: Vec<_> = (0..256u16).map(|i| [i as u8, 0, 0, 0]).collect();
    let image = Image::from_pixels(
        256,
        1,
        &pixels,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let stats = image.inspect(None, 256, false, &mut Work::default()).unwrap();
    assert_eq!(stats.unique_colors, 256);
    assert!(!stats.histogram_truncated);
    assert_eq!(stats.histogram.len(), 256);
    assert_eq!(stats.histogram[0].rgba, [0, 0, 0, 0]);
    assert_eq!(stats.histogram[255].rgba, [255, 0, 0, 0]);
}
