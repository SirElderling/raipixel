use raipixel::{Error, Image, Limits, Resource, Work};

fn rgba(width: u32, pixels: &[[u8; 4]]) -> Image {
    Image::from_pixels(
        width,
        (pixels.len() as u32) / width,
        pixels,
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
}

#[test]
fn exact_palette_extraction_counts_visible_rgb_and_preserves_hidden_bytes() {
    let image = rgba(
        3,
        &[
            [9, 8, 7, 0],
            [7, 8, 9, 255],
            [1, 2, 3, 128],
            [3, 2, 1, 255],
            [7, 8, 9, 20],
            [1, 2, 3, 0],
        ],
    );
    let baseline = image.try_copy(&mut Work::default()).unwrap();
    let colors = image
        .extract_visible_palette(256, &mut Work::default())
        .unwrap();
    assert_eq!(colors.len(), 3);
    assert_eq!(colors[0].rgb, [7, 8, 9]);
    assert_eq!(colors[0].count, 2);
    assert_eq!(colors[1].rgb, [1, 2, 3]);
    assert_eq!(colors[1].count, 1);
    assert_eq!(colors[2].rgb, [3, 2, 1]);
    assert_eq!(colors[2].count, 1);
    assert_eq!(image, baseline);
}

#[test]
fn empty_palette_reports_specific_error_even_when_hidden_rgb_is_nonzero() {
    let image = rgba(2, &[[10, 20, 30, 0], [40, 50, 60, 0]]);
    assert_eq!(
        image.extract_visible_palette(256, &mut Work::default()),
        Err(Error::PaletteEmpty)
    );
}

#[test]
fn overflow_reports_exact_distinct_colors_without_partial_palette() {
    let pixels: Vec<_> = (0..257u16)
        .map(|i| [(i & 255) as u8, (i >> 8) as u8, 17, 255])
        .collect();
    let image = rgba(257, &pixels);
    assert_eq!(
        image.extract_visible_palette(256, &mut Work::default()),
        Err(Error::PaletteTooMany {
            limit: 256,
            distinct: 257
        })
    );
    assert_eq!(
        image.extract_visible_palette(12, &mut Work::default()),
        Err(Error::PaletteTooMany {
            limit: 12,
            distinct: 257
        })
    );
}

#[test]
fn bounded_work_and_rejected_maximum_leave_input_unchanged() {
    let image = rgba(2, &[[1, 2, 3, 255], [4, 5, 6, 255]]);
    let mut work = Work::new(20).unwrap();
    assert_eq!(
        image.extract_visible_palette(256, &mut work),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(work.remaining(), 20);
    assert_eq!(
        image.extract_visible_palette(0, &mut work),
        Err(Error::PixelCount)
    );
    assert_eq!(work.remaining(), 20);
    assert_eq!(image.pixel(0, 0), Some([1, 2, 3, 255]));
}
