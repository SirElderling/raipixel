//! A complete static drawing/import/export slice without the future job CLI.
//! Reference expectations are generated from authored pixels, not the encoder.
use raipixel::{Image, Limits, Rect, Work, png};

const BORDER: [u8; 4] = [220, 30, 40, 255];
const HIDDEN: [u8; 4] = [20, 30, 40, 0];
const LINE: [u8; 4] = [4, 5, 6, 255];
const CORRECTION: [u8; 4] = [8, 9, 10, 0];

#[test]
fn exact_symbolic_artwork_survives_all_static_png_writers_and_reopen() {
    let mut work = Work::default();
    let mut image = Image::new(7, 7, Limits::default(), &mut work).unwrap();
    let frame = [
        "#######", "#.....#", "#.....#", "#.....#", "#.....#", "#.....#", "#######",
    ];
    assert_eq!(
        image.draw_grid(
            [0, 0],
            &frame,
            &[(b'#', Some(BORDER)), (b'.', None)],
            false,
            &mut work,
        ),
        Ok(0)
    );
    assert_eq!(image.flood_fill([3, 3], HIDDEN, None, &mut work), Ok(25));
    assert_eq!(image.line([1, 1], [5, 5], LINE, false, &mut work), Ok(0));
    assert_eq!(
        image.draw_grid(
            [2, 2],
            &["Aa", "aA"],
            &[(b'A', Some(CORRECTION)), (b'a', None)],
            false,
            &mut work,
        ),
        Ok(0)
    );

    // Independent per-pixel expectation: the expected image is not
    // reconstructed by running the implementation's drawing primitives.
    for y in 0..7 {
        for x in 0..7 {
            let expected = if x == 0 || y == 0 || x == 6 || y == 6 {
                BORDER
            } else if (x == 2 && y == 2) || (x == 3 && y == 3) {
                CORRECTION
            } else if x == y {
                LINE
            } else {
                HIDDEN
            };
            assert_eq!(image.pixel(x, y), Some(expected), "pixel ({x}, {y})");
        }
    }

    let mut outputs = Vec::new();
    let mut stored = Vec::new();
    png::encode(&image, &mut stored, &mut Work::default()).unwrap();
    outputs.push(stored);
    let mut compact = Vec::new();
    png::encode_compact(&image, &mut compact, &mut Work::default()).unwrap();
    outputs.push(compact);
    let mut indexed = Vec::new();
    assert!(png::encode_indexed(&image, &mut indexed, &mut Work::default()).unwrap());
    outputs.push(indexed);
    for bytes in &outputs {
        let reopened = png::decode(bytes, Limits::default(), &mut Work::default())
            .unwrap()
            .image;
        assert_eq!(reopened, image);
    }
    for (bytes, writer) in outputs.iter().zip(0..3) {
        let mut again = Vec::new();
        match writer {
            0 => png::encode(&image, &mut again, &mut Work::default()).unwrap(),
            1 => png::encode_compact(&image, &mut again, &mut Work::default()).unwrap(),
            _ => {
                assert!(png::encode_indexed(
                    &image,
                    &mut again,
                    &mut Work::default()
                )
                .unwrap());
            }
        }
        assert_eq!(*bytes, again, "nondeterministic PNG writer {writer}");
    }

    // A rejected edit cannot mutate the source before publication.
    let mut reopened = png::decode(&outputs[2], Limits::default(), &mut Work::default())
        .unwrap()
        .image;
    let before = reopened.try_copy(&mut Work::default()).unwrap();
    let mut limited_work = Work::new(5).unwrap();
    assert!(reopened
        .draw_grid(
            [1, 1],
            &["??"],
            &[(b'#', Some(LINE))],
            false,
            &mut limited_work,
        )
        .is_err());
    assert_eq!(limited_work.remaining(), 5);
    assert_eq!(reopened, before);

    // Bounds rejection likewise leaves the edited RGBA intact.
    assert!(reopened
        .fill(
            Rect::new(6, 6, 2, 2).unwrap(),
            LINE,
            false,
            &mut limited_work,
        )
        .is_err());
    assert_eq!(limited_work.remaining(), 5);
    assert_eq!(reopened, before);
}
