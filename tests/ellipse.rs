use raipixel::{Error, Image, Limits, Pixel, Rect, Resource, Work};

const CLEAR: Pixel = [0, 0, 0, 0];
const HIDDEN: Pixel = [13, 57, 101, 0];

fn image(width: u32, height: u32) -> Image {
    Image::new(width, height, Limits::default(), &mut Work::default()).unwrap()
}

// Independent oracle: double-size all integer pixel centers and apply the
// exact conic inequality prescribed in docs/design/05-functional-specification.md.
fn oracle(rect: [i32; 4], x: i32, y: i32) -> bool {
    let [rx, ry, w, h] = rect;
    if x < rx
        || y < ry
        || i64::from(x) >= i64::from(rx) + i64::from(w)
        || i64::from(y) >= i64::from(ry) + i64::from(h)
    {
        return false;
    }
    let width = i128::from(w);
    let height = i128::from(h);
    let horizontal = 2 * i128::from(x - rx) + 1 - width;
    let vertical = 2 * i128::from(y - ry) + 1 - height;
    horizontal.pow(2) * height.pow(2) + vertical.pow(2) * width.pow(2)
        <= width.pow(2) * height.pow(2)
}

#[test]
fn ellipse_matches_independent_integer_oracle_for_small_shapes_and_outlines() {
    for width in 1..=13 {
        for height in 1..=13 {
            let rect = Rect::new(2, 3, width, height).unwrap();
            for filled in [true, false] {
                let mut canvas = image(19, 20);
                let mut work = Work::new(u64::from(width) * u64::from(height)).unwrap();
                assert_eq!(
                    canvas.ellipse(rect, HIDDEN, filled, false, &mut work),
                    Ok(0)
                );
                assert_eq!(work.remaining(), 0);
                for y in 0..20 {
                    for x in 0..19 {
                        let x = x as i32;
                        let y = y as i32;
                        let expected = oracle([2, 3, width as i32, height as i32], x, y)
                            && (filled
                                || ![
                                    oracle([2, 3, width as i32, height as i32], x - 1, y),
                                    oracle([2, 3, width as i32, height as i32], x + 1, y),
                                    oracle([2, 3, width as i32, height as i32], x, y - 1),
                                    oracle([2, 3, width as i32, height as i32], x, y + 1),
                                ]
                                .into_iter()
                                .all(|inside| inside));
                        assert_eq!(
                            canvas.pixel(x as u32, y as u32),
                            Some(if expected { HIDDEN } else { CLEAR }),
                            "{width}x{height} ({x},{y}) filled={filled}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn clipped_ellipse_preserves_unselected_samples_and_declared_work() {
    let mut canvas = image(5, 5);
    let rect = Rect::new(-2, -1, 6, 6).unwrap();
    let mut budget = Work::new(36).unwrap();
    assert_eq!(
        canvas.ellipse(rect, HIDDEN, true, true, &mut budget),
        Ok(36 - 20)
    );
    assert_eq!(budget.remaining(), 0);
    for y in 0..5 {
        for x in 0..5 {
            assert_eq!(
                canvas.pixel(x, y),
                Some(if oracle([-2, -1, 6, 6], x as i32, y as i32) {
                    HIDDEN
                } else {
                    CLEAR
                })
            );
        }
    }
}

#[test]
fn malformed_ellipse_and_work_failure_never_mutate_pixels() {
    let mut canvas = image(4, 4);
    let mut work = Work::new(4).unwrap();
    assert_eq!(
        canvas.ellipse(
            Rect::new(-1, 0, 2, 2).unwrap(),
            HIDDEN,
            true,
            false,
            &mut work
        ),
        Err(Error::Bounds)
    );
    assert_eq!(
        canvas.ellipse(
            Rect::new(0, 0, 4, 4).unwrap(),
            HIDDEN,
            true,
            true,
            &mut work
        ),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(work.remaining(), 4);
    assert_eq!(canvas.pixels(), &[CLEAR; 16]);
    // An extreme declared rectangle must not overflow squared arithmetic.
    assert_eq!(
        canvas.ellipse(
            Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX).unwrap(),
            HIDDEN,
            false,
            true,
            &mut work
        ),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(canvas.pixels(), &[CLEAR; 16]);
}

#[test]
fn ellipse_center_and_perimeter_are_distinct_on_seven_square() {
    let mut filled = image(7, 7);
    let mut outline = image(7, 7);
    let rect = Rect::new(0, 0, 7, 7).unwrap();
    filled
        .ellipse(rect, HIDDEN, true, false, &mut Work::default())
        .unwrap();
    outline
        .ellipse(rect, HIDDEN, false, false, &mut Work::default())
        .unwrap();
    assert_eq!(filled.pixel(3, 3), Some(HIDDEN));
    assert_eq!(outline.pixel(3, 3), Some(CLEAR));
    assert_eq!(outline.pixel(3, 0), Some(HIDDEN));
    assert!(
        outline.pixels().iter().filter(|x| **x == HIDDEN).count()
            < filled.pixels().iter().filter(|x| **x == HIDDEN).count()
    );
}
