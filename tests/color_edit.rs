use raipixel::{Error, Image, Limits, Pixel, Rect, Resource, Work};

fn image(colors: &[Pixel]) -> Image {
    Image::from_pixels(
        colors.len() as u32,
        1,
        colors,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn simultaneous_mapping_does_not_cascade_and_preserves_hidden_rgb() {
    let a = [10, 20, 30, 0];
    let b = [40, 50, 60, 200];
    let c = [70, 80, 90, 255];
    let mut image = image(&[a, b, c, b]);
    let pairs = [(a, b), (b, c)];
    let mut work = Work::new(4).unwrap();
    assert_eq!(image.map_colors(&pairs, true, None, &mut work), Ok(3));
    assert_eq!(
        image.pixels(),
        &[[40, 50, 60, 0], [70, 80, 90, 200], c, [70, 80, 90, 200]]
    );
    assert_eq!(work.remaining(), 0);
}

#[test]
fn rgba_mapping_requires_exact_alpha_and_can_explicitly_change_it() {
    let mut image = image(&[[1, 2, 3, 0], [1, 2, 3, 255]]);
    let pairs = [([1, 2, 3, 0], [7, 8, 9, 0])];
    assert_eq!(
        image.map_colors(&pairs, false, None, &mut Work::default()),
        Ok(1)
    );
    assert_eq!(image.pixels(), &[[7, 8, 9, 0], [1, 2, 3, 255]]);
}

#[test]
fn region_bounds_and_work_errors_never_modify_pixels() {
    let original = [[1, 2, 3, 0]; 4];
    let mut image =
        Image::from_pixels(2, 2, &original, Limits::default(), &mut Work::default()).unwrap();
    let rect = Rect::new(1, 1, 1, 1).unwrap();
    assert_eq!(
        image.map_colors(
            &[([1, 2, 3, 255], [4, 5, 6, 255])],
            true,
            Some(rect),
            &mut Work::default()
        ),
        Ok(1)
    );
    assert_eq!(image.pixel(1, 1), Some([4, 5, 6, 0]));
    let snapshot = image.try_copy(&mut Work::default()).unwrap();
    let mut budget = Work::new(1).unwrap();
    assert_eq!(
        image.alpha_threshold(128, None, &mut budget),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(budget.remaining(), 1);
    assert_eq!(
        image.alpha_key(
            [1, 2, 3, 0],
            false,
            Some(Rect::new(-1, 0, 1, 1).unwrap()),
            &mut budget
        ),
        Err(Error::Bounds)
    );
    assert_eq!(image, snapshot);
}

#[test]
fn duplicate_mapping_sources_are_rejected_in_rgb_and_rgba_modes() {
    let mut image = image(&[[1, 2, 3, 0]]);
    let pairs = [
        ([1, 2, 3, 0], [4, 5, 6, 0]),
        ([1, 2, 3, 255], [7, 8, 9, 255]),
    ];
    let mut work = Work::new(1).unwrap();
    assert_eq!(
        image.map_colors(&pairs, true, None, &mut work),
        Err(Error::PixelCount)
    );
    assert_eq!(work.remaining(), 1);
    assert_eq!(image.pixels(), &[[1, 2, 3, 0]]);
    assert_eq!(image.map_colors(&pairs, false, None, &mut work), Ok(1));
}

#[test]
fn alpha_key_matches_requested_channels_and_preserves_rgb() {
    let base = [[8, 9, 10, 0], [8, 9, 10, 128], [8, 9, 10, 255]];
    let mut image = image(&base);
    assert_eq!(
        image.alpha_key([8, 9, 10, 128], true, None, &mut Work::default()),
        Ok(1)
    );
    assert_eq!(
        image.pixels(),
        &[[8, 9, 10, 0], [8, 9, 10, 0], [8, 9, 10, 255]]
    );
    assert_eq!(
        image.alpha_key([8, 9, 10, 0], false, None, &mut Work::default()),
        Ok(1)
    );
    assert_eq!(image.pixels(), &[[8, 9, 10, 0]; 3]);
}

#[test]
fn alpha_threshold_uses_strict_below_and_preserves_rgb() {
    let base = [[5, 6, 7, 0], [5, 6, 7, 127], [5, 6, 7, 128], [5, 6, 7, 255]];
    let mut image = image(&base);
    assert_eq!(image.alpha_threshold(128, None, &mut Work::default()), Ok(2));
    assert_eq!(
        image.pixels(),
        &[[5, 6, 7, 0], [5, 6, 7, 0], [5, 6, 7, 255], [5, 6, 7, 255]]
    );
}

#[test]
fn upper_mapping_bound_and_reverse_pair_order_are_deterministic() {
    let mut image = image(&[[8, 7, 6, 123]; 7]);
    let pairs: Vec<_> = (0..256)
        .map(|i| ([i as u8, 7, 6, 0], [255 - i as u8, 7, 6, 0]))
        .collect();
    let mut reversed = pairs.clone();
    reversed.reverse();
    let mut second = image.try_copy(&mut Work::default()).unwrap();
    assert_eq!(image.map_colors(&pairs, true, None, &mut Work::default()), Ok(7));
    assert_eq!(second.map_colors(&reversed, true, None, &mut Work::default()), Ok(7));
    assert_eq!(image, second);
    let mut excessive = pairs;
    excessive.push(([0, 0, 0, 0], [0, 0, 0, 0]));
    assert_eq!(
        image.map_colors(&excessive, true, None, &mut Work::default()),
        Err(Error::PixelCount)
    );
}
