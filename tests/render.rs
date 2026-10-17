use raipixel::{
    Image, Limits, Work, png,
    render::{self, Layer},
};

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
fn bottom_copy_preserves_all_hidden_rgb_and_png_round_trip() {
    let pixels = [[21, 42, 63, 0], [3, 6, 9, 255]];
    let source = image(2, 1, &pixels);
    let rendered = render::frame(
        [2, 1],
        &[Layer {
            image: &source,
            at: [0, 0],
            visible: true,
            opacity: 255,
        }],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    assert_eq!(rendered.pixels(), &pixels);
    assert_eq!(source.pixels(), &pixels);

    let mut png_bytes = Vec::new();
    png::encode(&rendered, &mut png_bytes, &mut Work::default()).unwrap();
    assert_eq!(
        png::decode(&png_bytes, Limits::default(), &mut Work::default())
            .unwrap()
            .image,
        rendered
    );
}

#[test]
fn upper_layers_use_integer_over_and_ignore_zero_alpha_samples() {
    let base = image(2, 1, &[[12, 34, 56, 0], [0, 0, 255, 255]]);
    let upper = image(2, 1, &[[99, 88, 77, 0], [255, 0, 0, 128]]);
    let hidden = image(2, 1, &[[7, 7, 7, 255]; 2]);
    let rendered = render::frame(
        [2, 1],
        &[
            Layer {
                image: &hidden,
                at: [0, 0],
                visible: false,
                opacity: 255,
            },
            Layer {
                image: &base,
                at: [0, 0],
                visible: true,
                opacity: 255,
            },
            Layer {
                image: &upper,
                at: [0, 0],
                visible: true,
                opacity: 255,
            },
        ],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    assert_eq!(rendered.pixels(), &[[12, 34, 56, 0], [128, 0, 127, 255]]);
    assert_eq!(base.pixels()[0], [12, 34, 56, 0]);
    assert_eq!(upper.pixels()[0], [99, 88, 77, 0]);
}

#[test]
fn first_visible_layer_copy_applies_opacity_and_clipping() {
    let source = image(3, 1, &[[8, 9, 10, 255], [44, 55, 66, 101], [9, 8, 7, 0]]);
    let rendered = render::frame(
        [2, 1],
        &[Layer {
            image: &source,
            at: [-1, 0],
            visible: true,
            opacity: 0,
        }],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    assert_eq!(rendered.pixels(), &[[44, 55, 66, 0], [9, 8, 7, 0]]);
}

#[test]
fn empty_or_invisible_layers_produce_a_clear_canvas() {
    let hidden = image(1, 1, &[[1, 2, 3, 255]]);
    for layers in [
        &[][..],
        &[Layer {
            image: &hidden,
            at: [0, 0],
            visible: false,
            opacity: 255,
        }][..],
    ] {
        let result =
            render::frame([2, 2], layers, Limits::default(), &mut Work::default()).unwrap();
        assert_eq!(result.pixels(), &[[0; 4]; 4]);
    }
}

#[test]
fn renderer_rejects_limits_and_budget_without_mutating_cels() {
    let original = [[11, 22, 33, 0]; 4];
    let source = image(2, 2, &original);
    let layer = Layer {
        image: &source,
        at: [0, 0],
        visible: true,
        opacity: 255,
    };
    let mut insufficient = Work::new(7).unwrap(); // 4 output pixels + 4 declared stamps.
    assert_eq!(
        render::frame([2, 2], &[layer], Limits::default(), &mut insufficient),
        Err(raipixel::Error::Limit(raipixel::Resource::Work))
    );
    assert_eq!(source.pixels(), &original);
    let small = Limits::new(2, 3, 12).unwrap();
    assert_eq!(
        render::frame([2, 2], &[], small, &mut Work::default()),
        Err(raipixel::Error::Limit(raipixel::Resource::Pixels))
    );
    assert_eq!(source.pixels(), &original);
}
