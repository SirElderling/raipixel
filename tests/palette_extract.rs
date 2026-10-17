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

#[test]
fn counted_path_matches_small_sorted_path_and_charges_exact_work() {
    let small: Vec<_> = (0..16u32 * 16)
        .map(|i| [(i % 31) as u8, 19, 29, if i % 13 == 0 { 0 } else { 255 }])
        .collect();
    let mut large = Vec::new();
    for _ in 0..16 {
        large.extend_from_slice(&small);
    }
    let source_small = rgba(16, &small);
    let source_large = rgba(64, &large);
    let mut a = Work::default();
    let mut b = Work::default();
    let one = source_small.extract_visible_palette(256, &mut a).unwrap();
    let two = source_large.extract_visible_palette(256, &mut b).unwrap();
    assert_eq!(one.len(), two.len());
    for (a, b) in one.iter().zip(two.iter()) {
        assert_eq!(a.rgb, b.rgb);
        assert_eq!(b.count, a.count * 16);
    }
    assert_eq!(Work::default().remaining() - a.remaining(), 16 * 16 * 32);
    assert_eq!(Work::default().remaining() - b.remaining(), 64 * 64 * 32);
}

#[test]
fn counted_overflow_reports_actual_distinct_count_and_preserves_bytes() {
    let colors: Vec<_> = (0..4096u32)
        .map(|i| [(i % 251) as u8, ((i / 251) % 7) as u8, 0, 255])
        .collect();
    let image = rgba(64, &colors);
    let prior = image.try_copy(&mut Work::default()).unwrap();
    let mut work = Work::default();
    assert_eq!(
        image.extract_visible_palette(12, &mut work),
        Err(Error::PaletteTooMany {
            limit: 12,
            distinct: 1757
        })
    );
    assert_eq!(
        Work::default().remaining() - work.remaining(),
        4096 * 32
    );
    assert_eq!(image, prior);
}

#[test]
fn constrained_bytes_use_compatible_sort_fallback_or_reject_before_work() {
    let pixels = vec![[3, 2, 1, 255]; 4096];
    // 4 * 4096 live RGBA plus 3 * 4096 sortable bytes fit; the
    // fixed hash scratch does not. The prior implementation also succeeded.
    let limited = Limits::new(64, 4096, 30_000).unwrap();
    let image =
        Image::from_pixels(64, 64, &pixels, limited, &mut Work::default()).unwrap();
    assert_eq!(
        image.extract_visible_palette(256, &mut Work::default())
            .unwrap()[0].count,
        4096
    );
    let tighter = Limits::new(64, 4096, 20_000).unwrap();
    let image =
        Image::from_pixels(64, 64, &pixels, tighter, &mut Work::default()).unwrap();
    let mut work = Work::default();
    assert_eq!(
        image.extract_visible_palette(256, &mut work),
        Err(Error::Limit(Resource::Bytes))
    );
    assert_eq!(work.remaining(), Work::default().remaining());
}

#[test]
fn adversarial_32_probe_collision_falls_back_without_losing_counts() {
    // For this exact hash, red is absent from the low 11-bit slot key.
    let pixels: Vec<_> = (0..4096u32)
        .map(|i| [(i % 128) as u8, 0, 0, 255])
        .collect();
    let image = rgba(64, &pixels);
    let palette = image
        .extract_visible_palette(256, &mut Work::default())
        .unwrap();
    assert_eq!(palette.len(), 128);
    assert!(palette.iter().all(|c| c.count == 32));
    assert_eq!(palette[0].rgb, [0, 0, 0]);
    assert_eq!(palette[127].rgb, [127, 0, 0]);
}
