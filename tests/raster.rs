use raipixel::{Blend, Error, Image, Limits, Rect, Resource, Work, blend};

const CLEAR: [u8; 4] = [0; 4];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn work() -> Work {
    Work::default()
}
fn image(w: u32, h: u32) -> Image {
    Image::new(w, h, Limits::default(), &mut work()).unwrap()
}
fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}

#[test]
fn images_are_transparent_row_major_and_preserve_hidden_rgb() {
    let mut img = image(2, 2);
    assert_eq!(img.pixels(), &[CLEAR; 4]);
    img.fill(rect(1, 0, 1, 1), [9, 8, 7, 0], false, &mut work())
        .unwrap();
    assert_eq!(img.size(), [2, 2]);
    assert_eq!(img.pixels(), &[CLEAR, [9, 8, 7, 0], CLEAR, CLEAR]);
    assert_eq!(img.pixel(1, 0), Some([9, 8, 7, 0]));
    assert_eq!(img.pixel(2, 0), None);
    assert_eq!(img.pixel(0, u32::MAX), None);
}

#[test]
fn rejects_zero_and_oversized_images_before_allocating() {
    for (w, h) in [(0, 1), (1, 0), (4097, 1), (u32::MAX, u32::MAX)] {
        assert!(Image::new(w, h, Limits::default(), &mut work()).is_err());
    }
    assert_eq!(
        Image::new(0, 1, Limits::default(), &mut work()).unwrap_err(),
        Error::Dimensions
    );
}

#[test]
fn limits_can_only_be_lowered_and_are_inclusive() {
    let limits = Limits::new(2, 3, 12).unwrap();
    assert!(Image::new(1, 3, limits, &mut work()).is_err());
    assert!(Image::new(2, 2, limits, &mut work()).is_err());
    assert!(Image::new(1, 2, Limits::new(2, 4, 7).unwrap(), &mut work()).is_err());
    assert_eq!(
        Image::new(1, 2, Limits::new(2, 2, 8).unwrap(), &mut work())
            .unwrap()
            .pixels(),
        &[CLEAR; 2]
    );
    for values in [
        (4097, 1, 4),
        (1, 16_777_217, 4),
        (1, 1, 536_870_913),
        (0, 1, 4),
    ] {
        assert!(Limits::new(values.0, values.1, values.2).is_err());
    }
}

#[test]
fn pixel_import_requires_exact_count_and_copies_samples() {
    let mut pixels = [[1, 2, 3, 0], BLUE];
    let img = Image::from_pixels(2, 1, &pixels, Limits::default(), &mut work()).unwrap();
    pixels[0] = RED;
    assert_eq!(img.pixels(), &[[1, 2, 3, 0], BLUE]);
    assert_eq!(
        Image::from_pixels(1, 1, &pixels, Limits::default(), &mut work()).unwrap_err(),
        Error::PixelCount
    );
}

#[test]
fn rectangles_reject_empty_sizes_but_allow_wide_endpoints() {
    assert_eq!(Rect::new(0, 0, 0, 1), Err(Error::Rectangle));
    assert_eq!(Rect::new(0, 0, 1, 0), Err(Error::Rectangle));
    assert!(Rect::new(i32::MAX, i32::MAX, u32::MAX, u32::MAX).is_ok());
}

#[test]
fn fill_is_half_open_and_preserves_unselected_samples() {
    let mut img =
        Image::from_pixels(3, 2, &[[7, 8, 9, 0]; 6], Limits::default(), &mut work()).unwrap();
    img.fill(rect(1, 0, 1, 2), RED, false, &mut work()).unwrap();
    assert_eq!(
        img.pixels(),
        &[
            [7, 8, 9, 0],
            RED,
            [7, 8, 9, 0],
            [7, 8, 9, 0],
            RED,
            [7, 8, 9, 0]
        ]
    );
}

#[test]
fn out_of_bounds_fill_fails_without_mutation() {
    for r in [rect(-1, 0, 2, 1), rect(1, 1, 2, 1), rect(i32::MAX, 0, 1, 1)] {
        let mut img = image(2, 2);
        assert_eq!(img.fill(r, RED, false, &mut work()), Err(Error::Bounds));
        assert_eq!(img.pixels(), &[CLEAR; 4]);
    }
}

#[test]
fn clipping_intersects_all_edges_and_reports_discarded_pixels() {
    let mut img = image(2, 2);
    assert_eq!(
        img.fill(rect(-1, -1, 4, 4), RED, true, &mut work())
            .unwrap(),
        12
    );
    assert_eq!(img.pixels(), &[RED; 4]);
    assert_eq!(
        img.fill(rect(2, 0, 1, 2), BLUE, true, &mut work()).unwrap(),
        2
    );
    assert_eq!(img.pixels(), &[RED; 4]);
}

#[test]
fn huge_clipped_geometry_is_charged_before_traversal() {
    let mut img = image(1, 1);
    let mut budget = Work::new(10).unwrap();
    assert_eq!(
        img.fill(rect(i32::MIN, 0, u32::MAX, 1), RED, true, &mut budget),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(img.pixels(), &[CLEAR]);
    assert_eq!(budget.remaining(), 10);
}

#[test]
fn work_is_cumulative_and_failed_edits_do_not_write() {
    let mut img = image(2, 2);
    let mut budget = Work::new(5).unwrap();
    img.fill(rect(0, 0, 2, 2), RED, false, &mut budget).unwrap();
    assert_eq!(budget.remaining(), 1);
    assert_eq!(
        img.fill(rect(0, 0, 2, 1), BLUE, false, &mut budget),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(img.pixels(), &[RED; 4]);
    assert_eq!(budget.remaining(), 1);
    assert!(Work::new(1_000_000_001).is_err());
}

#[test]
fn creation_and_import_charge_pixel_visits() {
    assert!(Image::new(2, 2, Limits::default(), &mut Work::new(3).unwrap()).is_err());
    assert!(
        Image::from_pixels(
            2,
            2,
            &[RED; 4],
            Limits::default(),
            &mut Work::new(3).unwrap()
        )
        .is_err()
    );
}

#[test]
fn copy_replaces_all_channels_including_transparent_clears() {
    let src =
        Image::from_pixels(2, 1, &[[4, 5, 6, 0], BLUE], Limits::default(), &mut work()).unwrap();
    let mut dst = image(3, 1);
    dst.fill(rect(0, 0, 3, 1), RED, false, &mut work()).unwrap();
    dst.stamp(
        &src,
        rect(0, 0, 2, 1),
        [1, 0],
        Blend::Copy,
        255,
        false,
        &mut work(),
    )
    .unwrap();
    assert_eq!(dst.pixels(), &[RED, [4, 5, 6, 0], BLUE]);
}

#[test]
fn source_rect_must_be_in_bounds_even_when_destination_clips() {
    let src = image(2, 1);
    let mut dst = image(2, 1);
    assert_eq!(
        dst.stamp(
            &src,
            rect(-1, 0, 2, 1),
            [0, 0],
            Blend::Copy,
            255,
            true,
            &mut work()
        ),
        Err(Error::Bounds)
    );
    assert_eq!(dst.pixels(), &[CLEAR; 2]);
}

#[test]
fn clipped_stamp_keeps_source_alignment() {
    let src = Image::from_pixels(
        3,
        2,
        &[RED, BLUE, CLEAR, BLUE, RED, [9, 8, 7, 0]],
        Limits::default(),
        &mut work(),
    )
    .unwrap();
    let mut dst = image(2, 1);
    assert_eq!(
        dst.stamp(
            &src,
            rect(0, 0, 3, 2),
            [-1, -1],
            Blend::Copy,
            255,
            true,
            &mut work()
        )
        .unwrap(),
        4
    );
    assert_eq!(dst.pixels(), &[RED, [9, 8, 7, 0]]);
}

#[test]
fn stamp_bounds_and_work_failures_preserve_destination() {
    let src = image(2, 2);
    let mut dst = image(2, 2);
    dst.fill(rect(0, 0, 2, 2), RED, false, &mut work()).unwrap();
    assert_eq!(
        dst.stamp(
            &src,
            rect(0, 0, 2, 2),
            [1, 0],
            Blend::Copy,
            255,
            false,
            &mut work()
        ),
        Err(Error::Bounds)
    );
    assert_eq!(
        dst.stamp(
            &src,
            rect(0, 0, 2, 2),
            [0, 0],
            Blend::Copy,
            255,
            false,
            &mut Work::new(3).unwrap()
        ),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(dst.pixels(), &[RED; 4]);
}

#[test]
fn overlapping_copy_reads_pre_edit_samples_in_both_directions() {
    for (source, dest, expected) in [
        (rect(0, 0, 3, 1), [1, 0], [RED, RED, BLUE, CLEAR]),
        (
            rect(1, 0, 3, 1),
            [0, 0],
            [BLUE, CLEAR, [9, 8, 7, 0], [9, 8, 7, 0]],
        ),
    ] {
        let mut img = Image::from_pixels(
            4,
            1,
            &[RED, BLUE, CLEAR, [9, 8, 7, 0]],
            Limits::default(),
            &mut work(),
        )
        .unwrap();
        img.stamp_within(source, dest, Blend::Copy, 255, false, &mut work())
            .unwrap();
        assert_eq!(img.pixels(), &expected);
    }
}

#[test]
fn overlapping_copy_charges_snapshot_before_writing() {
    let mut img = image(2, 1);
    img.fill(rect(0, 0, 2, 1), RED, false, &mut work()).unwrap();
    assert_eq!(
        img.stamp_within(
            rect(0, 0, 2, 1),
            [0, 0],
            Blend::Copy,
            255,
            false,
            &mut Work::new(3).unwrap()
        ),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(img.pixels(), &[RED; 2]);
}

#[test]
fn fallible_copy_is_independent_and_preserves_hidden_samples() {
    let src =
        Image::from_pixels(2, 1, &[[3, 4, 5, 0], BLUE], Limits::default(), &mut work()).unwrap();
    let mut dst = src.try_copy(&mut work()).unwrap();
    dst.fill(rect(0, 0, 1, 1), RED, false, &mut work()).unwrap();
    assert_eq!(src.pixels(), &[[3, 4, 5, 0], BLUE]);
    assert_eq!(dst.pixels(), &[RED, BLUE]);
    assert!(src.try_copy(&mut Work::new(1).unwrap()).is_err());
}

#[test]
fn copy_opacity_retains_rgb_even_at_zero_alpha() {
    assert_eq!(blend([9, 8, 7, 128], RED, 128, Blend::Copy), [9, 8, 7, 64]);
    assert_eq!(blend([9, 8, 7, 0], RED, 255, Blend::Copy), [9, 8, 7, 0]);
    assert_eq!(blend(BLUE, RED, 0, Blend::Copy), [0, 0, 255, 0]);
}

#[test]
fn source_over_vectors_follow_integer_rounding_contract() {
    for (src, dst, opacity, expected) in [
        ([255, 0, 0, 128], BLUE, 255, [128, 0, 127, 255]),
        ([255, 0, 0, 128], [0, 0, 255, 128], 255, [170, 0, 85, 192]),
        ([255, 0, 0, 255], BLUE, 128, [128, 0, 127, 255]),
        ([9, 8, 7, 0], [4, 5, 6, 0], 255, [4, 5, 6, 0]),
        ([9, 8, 7, 128], [4, 5, 6, 0], 255, [9, 8, 7, 128]),
        (RED, BLUE, 255, RED),
        (RED, BLUE, 0, BLUE),
    ] {
        assert_eq!(blend(src, dst, opacity, Blend::Over), expected);
    }
}

#[test]
fn all_alpha_opacity_pairs_obey_opaque_destination_contract() {
    for alpha in 0..=255u8 {
        for opacity in 0..=255u8 {
            // Red over opaque black: effective alpha is also the exact red output.
            let effective = ((u32::from(alpha) * u32::from(opacity) + 127) / 255) as u8;
            assert_eq!(
                blend([255, 0, 0, alpha], [0, 0, 0, 255], opacity, Blend::Over),
                [effective, 0, 0, 255]
            );
        }
    }
}

#[test]
fn error_codes_match_proposed_interface_categories() {
    for (err, code) in [
        (Error::Dimensions, "E_ARGUMENT"),
        (Error::Rectangle, "E_ARGUMENT"),
        (Error::PixelCount, "E_ARGUMENT"),
        (Error::Bounds, "E_BOUNDS"),
        (Error::Limit(Resource::Work), "E_LIMIT"),
        (Error::Allocation, "E_ALLOC"),
    ] {
        assert_eq!(err.code(), code);
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn bounded_line_matches_independent_canonical_bresenham_in_all_octants() {
    // Step-by-step oracle from the design's integer error recurrence. The
    // implementation under test does not iterate off-canvas steps.
    fn oracle(mut a: [i32; 2], mut b: [i32; 2]) -> Vec<[i32; 2]> {
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        let dx = i64::from(b[0]) - i64::from(a[0]);
        let dy = -(i64::from(b[1]) - i64::from(a[1])).abs();
        let sy = if a[1] < b[1] { 1 } else { -1 };
        let (mut x, mut y) = (i64::from(a[0]), i64::from(a[1]));
        let (mut err, mut result) = (dx + dy, Vec::new());
        loop {
            result.push([x as i32, y as i32]);
            if x == i64::from(b[0]) && y == i64::from(b[1]) {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += 1;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
        result
    }

    let color = [31, 97, 201, 0]; // Hidden RGB must be retained.
    for x0 in -3..=4 {
        for y0 in -3..=4 {
            for x1 in -3..=4 {
                for y1 in -3..=4 {
                    let start = [x0, y0];
                    let end = [x1, y1];
                    let reference = oracle(start, end);
                    let mut expected = vec![CLEAR; 49];
                    let mut kept = 0u64;
                    for [x, y] in &reference {
                        if (0..7).contains(x) && (0..7).contains(y) {
                            expected[*y as usize * 7 + *x as usize] = color;
                            kept += 1;
                        }
                    }
                    let mut actual = image(7, 7);
                    let mut budget = Work::new(reference.len() as u64).unwrap();
                    assert_eq!(
                        actual
                            .line(start, end, color, true, &mut budget)
                            .unwrap(),
                        reference.len() as u64 - kept,
                        "{start:?} to {end:?}"
                    );
                    assert_eq!(budget.remaining(), 0);
                    assert_eq!(actual.pixels(), expected, "{start:?} to {end:?}");

                    let mut reversed = image(7, 7);
                    reversed
                        .line(end, start, color, true, &mut work())
                        .unwrap();
                    assert_eq!(reversed, actual, "endpoint order changed pixels");
                }
            }
        }
    }
}

#[test]
fn clipped_lines_have_bounded_runtime_and_exact_work_accounting() {
    let mut actual = image(9, 3);
    let mut budget = Work::new(2_000_001).unwrap();
    assert_eq!(
        actual
            .line([-1_000_000, 1], [1_000_000, 1], BLUE, true, &mut budget)
            .unwrap(),
        2_000_001 - 9
    );
    assert_eq!(budget.remaining(), 0);
    assert_eq!(actual.pixels()[9..18], [BLUE; 9]);
    assert_eq!(actual.pixel(0, 0), Some(CLEAR));

    let mut unchanged = image(9, 3);
    let mut budget = Work::new(10).unwrap();
    assert_eq!(
        unchanged
            .line([i32::MIN, 1], [i32::MAX, 1], RED, true, &mut budget)
            .unwrap_err(),
        Error::Limit(Resource::Work)
    );
    assert_eq!(budget.remaining(), 10);
    assert_eq!(unchanged.pixels(), &[CLEAR; 27]);
}

#[test]
fn unclipped_line_bounds_and_single_pixel_are_transactional() {
    let mut actual = image(4, 4);
    let mut budget = Work::new(2).unwrap();
    assert_eq!(
        actual
            .line([-1, 0], [2, 0], RED, false, &mut budget)
            .unwrap_err(),
        Error::Bounds
    );
    assert_eq!(budget.remaining(), 2);
    assert_eq!(actual.pixels(), &[CLEAR; 16]);

    let hidden = [43, 27, 99, 0];
    assert_eq!(
        actual.line([2, 3], [2, 3], hidden, false, &mut budget),
        Ok(0)
    );
    assert_eq!(budget.remaining(), 1);
    assert_eq!(actual.pixel(2, 3), Some(hidden));
    assert_eq!(
        actual.line([-1, -1], [-1, -1], RED, true, &mut budget),
        Ok(1)
    );
    assert_eq!(budget.remaining(), 0);
    assert_eq!(actual.pixel(2, 3), Some(hidden));
}

#[test]
fn flood_fill_is_four_connected_and_uses_exact_hidden_rgba() {
    let old = [9, 8, 7, 0];
    let different = [0, 0, 0, 0];
    let mut canvas = Image::from_pixels(
        3,
        3,
        &[old, different, old, old, different, old, old, old, old],
        Limits::default(),
        &mut work(),
    )
    .unwrap();
    let new_color = [21, 22, 23, 0];
    assert_eq!(
        canvas.flood_fill([0, 0], new_color, None, &mut work()),
        Ok(3)
    );
    assert_eq!(
        canvas.pixels(),
        &[new_color, different, old, new_color, different, old, new_color, old, old]
    );
    assert_eq!(
        canvas.flood_fill([0, 0], new_color, None, &mut work()),
        Ok(0)
    );
}

#[test]
fn flood_fill_respects_region_and_leaves_surroundings_untouched() {
    let mut canvas = image(5, 4);
    let region = rect(1, 1, 3, 2);
    assert_eq!(
        canvas.flood_fill([1, 1], BLUE, Some(region), &mut work()),
        Ok(6)
    );
    for y in 0..4 {
        for x in 0..5 {
            assert_eq!(
                canvas.pixel(x, y),
                Some(if (1..4).contains(&x) && (1..3).contains(&y) {
                    BLUE
                } else {
                    CLEAR
                })
            );
        }
    }
    assert_eq!(
        canvas.flood_fill([0, 0], RED, Some(region), &mut work()),
        Err(Error::Bounds)
    );
    assert_eq!(canvas.pixel(2, 2), Some(BLUE));
}

#[test]
fn flood_fill_work_exhaustion_and_memory_rejection_are_transactional() {
    let mut canvas = image(6, 6);
    let mut insufficient = Work::new(5).unwrap();
    assert_eq!(
        canvas.flood_fill([1, 1], RED, None, &mut insufficient),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(canvas.pixels(), &[CLEAR; 36]);

    let limits = Limits::new(6, 36, 144).unwrap();
    let mut memory_limited = Image::new(6, 6, limits, &mut work()).unwrap();
    let mut budget = Work::new(100).unwrap();
    assert_eq!(
        memory_limited.flood_fill([1, 1], BLUE, None, &mut budget),
        Err(Error::Limit(Resource::Bytes))
    );
    assert_eq!(budget.remaining(), 100);
    assert_eq!(memory_limited.pixels(), &[CLEAR; 36]);
}

#[test]
fn flood_fill_bounds_rejection_preserves_work_and_pixels() {
    let mut canvas = image(2, 2);
    let mut budget = Work::new(20).unwrap();
    assert_eq!(
        canvas.flood_fill([2, 0], RED, None, &mut budget),
        Err(Error::Bounds)
    );
    assert_eq!(
        canvas.flood_fill([1, 1], RED, Some(rect(-1, 0, 2, 2)), &mut budget),
        Err(Error::Bounds)
    );
    assert_eq!(budget.remaining(), 20);
    assert_eq!(canvas.pixels(), &[CLEAR; 4]);
}
