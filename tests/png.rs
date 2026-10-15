use raipixel::{
    Image, Limits, Work,
    json::{self, Value},
    png,
};
use std::collections::BTreeMap;

fn text<'a>(object: &'a BTreeMap<String, Value>, key: &str) -> &'a str {
    let Value::String(value) = &object[key] else {
        panic!("string expected")
    };
    value
}
fn number(object: &BTreeMap<String, Value>, key: &str) -> u64 {
    let Value::Number(value) = &object[key] else {
        panic!("number expected")
    };
    value.as_u64().unwrap()
}
fn boolean(object: &BTreeMap<String, Value>, key: &str) -> bool {
    let Value::Bool(value) = object[key] else {
        panic!("bool expected")
    };
    value
}
fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn fixtures() -> BTreeMap<String, Value> {
    let Value::Object(object) = json::parse(
        include_bytes!("fixtures/png.json"),
        json::Limits::default(),
        &mut Work::default(),
    )
    .unwrap() else {
        panic!("object expected")
    };
    object
}

#[test]
fn independently_generated_pngs_cover_the_required_profile() {
    let fixtures = fixtures();
    let Value::Array(cases) = &fixtures["valid"] else {
        panic!("array expected")
    };
    for value in cases {
        let Value::Object(case) = value else {
            panic!("object expected")
        };
        let name = text(case, "name");
        let decoded = png::decode(
            &hex(text(case, "png")),
            Limits::default(),
            &mut Work::default(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            decoded.image.size(),
            [number(case, "width") as u32, number(case, "height") as u32],
            "{name}"
        );
        assert_eq!(
            decoded
                .image
                .pixels()
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
            hex(text(case, "rgba")),
            "{name}"
        );
        assert_eq!(
            decoded.uninterpreted_color,
            boolean(case, "color_warning"),
            "{name}"
        );
        assert_eq!(
            decoded.skipped_ancillary,
            boolean(case, "ancillary_warning"),
            "{name}"
        );
        assert_eq!(
            decoded.trailing_idat_bytes,
            number(case, "trailing") as usize,
            "{name}"
        );
    }
}

#[test]
fn rejects_malformed_and_unsupported_pngs_with_distinct_codes() {
    let fixtures = fixtures();
    let Value::Array(cases) = &fixtures["invalid"] else {
        panic!("array expected")
    };
    for value in cases {
        let Value::Object(case) = value else {
            panic!("object expected")
        };
        let err = png::decode(
            &hex(text(case, "png")),
            Limits::default(),
            &mut Work::default(),
        )
        .unwrap_err();
        assert_eq!(
            err.code(),
            text(case, "code"),
            "{}: {err}",
            text(case, "name")
        );
    }
}

#[test]
fn writer_matches_independently_constructed_canonical_bytes() {
    let image = Image::from_pixels(
        2,
        1,
        &[[1, 2, 3, 0], [4, 5, 6, 255]],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let mut output = Vec::new();
    png::encode(&image, &mut output, &mut Work::default()).unwrap();
    assert_eq!(output, hex(text(&fixtures(), "writer")));
}

#[test]
fn writer_handles_multiple_stored_blocks_and_preserves_hidden_rgb() {
    let pixels: Vec<_> = (0..128 * 128)
        .map(|i| [(i % 256) as u8, 17, 239, if i % 3 == 0 { 0 } else { 255 }])
        .collect();
    let image =
        Image::from_pixels(128, 128, &pixels, Limits::default(), &mut Work::default()).unwrap();
    let mut output = Vec::new();
    png::encode(&image, &mut output, &mut Work::default()).unwrap();
    let result = png::decode(&output, Limits::default(), &mut Work::default()).unwrap();
    assert_eq!(result.image, image);
    let mut pos = 8;
    let mut idat_count = 0;
    while pos < output.len() {
        let len = u32::from_be_bytes(output[pos..pos + 4].try_into().unwrap()) as usize;
        if &output[pos + 4..pos + 8] == b"IDAT" {
            assert!(len <= 65_540);
            idat_count += 1;
        }
        pos += 12 + len;
    }
    assert_eq!(idat_count, 4); // zlib header, two stored blocks, Adler checksum.
}

#[test]
fn limits_are_enforced_before_output_or_large_decode_allocations() {
    let image = Image::new(1, 1, Limits::default(), &mut Work::default()).unwrap();
    let mut output = Vec::new();
    assert_eq!(
        png::encode(&image, &mut output, &mut Work::new(0).unwrap())
            .unwrap_err()
            .code(),
        "E_LIMIT"
    );
    assert!(output.is_empty());
    let fixture = fixtures();
    let Value::Array(cases) = &fixture["valid"] else {
        panic!("array expected")
    };
    let Value::Object(case) = &cases[0] else {
        panic!("object expected")
    };
    let input = hex(text(case, "png"));
    assert_eq!(
        png::decode(&input, Limits::new(1, 1, 4).unwrap(), &mut Work::default())
            .unwrap_err()
            .code(),
        "E_LIMIT"
    );
    assert_eq!(
        png::decode(&input, Limits::default(), &mut Work::new(0).unwrap())
            .unwrap_err()
            .code(),
        "E_LIMIT"
    );
}

#[test]
fn writer_propagates_io_failure() {
    struct Fails;
    impl std::io::Write for Fails {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "test sink",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let image = Image::new(1, 1, Limits::default(), &mut Work::default()).unwrap();
    assert_eq!(
        png::encode(&image, &mut Fails, &mut Work::default())
            .unwrap_err()
            .code(),
        "E_IO"
    );
}
