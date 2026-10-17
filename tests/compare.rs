use raipixel::{Error, Image, Limits, Rect, Resource, Work};

fn image(width: u32, height: u32, colors: &[[u8; 4]]) -> Image {
    Image::from_pixels(
        width,
        height,
        colors,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn compare_distinguishes_hidden_rgb_visible_color_and_alpha() {
    let first = image(
        3,
        2,
        &[
            [1, 2, 3, 0],
            [4, 5, 6, 255],
            [6, 6, 6, 255],
            [8, 9, 10, 0],
            [11, 12, 13, 90],
            [14, 15, 16, 0],
        ],
    );
    let second = image(
        3,
        2,
        &[
            [3, 2, 1, 0],
            [4, 5, 6, 90],
            [7, 6, 6, 255],
            [8, 9, 10, 255],
            [11, 12, 13, 90],
            [14, 15, 16, 0],
        ],
    );
    let mut work = Work::new(6).unwrap();
    let result = first.compare(&second, None, &mut work).unwrap();
    assert_eq!(result.region, [0, 0, 3, 2]);
    assert_eq!(result.changed_samples, 4);
    assert_eq!(result.changed_visible_colors, 2);
    assert_eq!(result.changed_alpha, 2);
    assert_eq!(result.changed_bbox, Some([0, 0, 3, 2]));
    assert_eq!(work.remaining(), 0);
    assert_eq!(first.compare(&first, None, &mut Work::default()).unwrap().changed_bbox, None);
}

#[test]
fn compare_respects_exact_region_and_no_mutation() {
    let mut second_pixels = [[7, 8, 9, 0]; 16];
    second_pixels[0] = [1, 1, 1, 0];
    second_pixels[10] = [7, 8, 9, 255];
    second_pixels[15] = [1, 1, 1, 0];
    let first = image(4, 4, &[[7, 8, 9, 0]; 16]);
    let second = image(4, 4, &second_pixels);
    let original = second.try_copy(&mut Work::default()).unwrap();
    let region = Rect::new(1, 1, 2, 2).unwrap();
    let mut work = Work::new(4).unwrap();
    let change = first.compare(&second, Some(region), &mut work).unwrap();
    assert_eq!(change.changed_samples, 1);
    assert_eq!(change.changed_alpha, 1);
    assert_eq!(change.changed_visible_colors, 1);
    assert_eq!(change.changed_bbox, Some([2, 2, 1, 1]));
    assert_eq!(work.remaining(), 0);
    assert_eq!(original, second);
}

#[test]
fn compare_preflight_error_leaves_work_and_inputs_unchanged() {
    let first = image(2, 2, &[[0; 4]; 4]);
    let different_size = image(1, 2, &[[0; 4]; 2]);
    let mut work = Work::new(3).unwrap();
    assert_eq!(first.compare(&different_size, None, &mut work), Err(Error::Dimensions));
    assert_eq!(
        first.compare(
            &first,
            Some(Rect::new(-1, 0, 1, 1).unwrap()),
            &mut work
        ),
        Err(Error::Bounds)
    );
    assert_eq!(
        first.compare(&first, None, &mut work),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(work.remaining(), 3);
    assert_eq!(first.pixels(), &[[0; 4]; 4]);
}
