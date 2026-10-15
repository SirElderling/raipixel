// Independent black-box comparison harness. Uses public APIs only.
use raipixel::{Limits, Work};
use std::{
    hint::black_box,
    io::{Cursor, Write},
    time::Instant,
};
type AnyError = Box<dyn std::error::Error>;
fn reference_decode(data: &[u8]) -> Result<(u32, u32, Vec<u8>), AnyError> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.ignore_checksums(false);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let mut output = vec![0; reader.output_buffer_size().ok_or("output too large")?];
    let info = reader.next_frame(&mut output)?;
    output.truncate(info.buffer_size());
    reader.finish()?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err("not 8-bit normalized output".into());
    }
    let rgba = match info.color_type {
        png::ColorType::Rgba => output,
        png::ColorType::Rgb => output
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => output.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => output
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        _ => return Err("unexpanded palette".into()),
    };
    Ok((info.width, info.height, rgba))
}
fn reference_encode(
    output: impl Write,
    pixels: &[u8],
    width: u32,
    height: u32,
    mode: &str,
) -> Result<(), AnyError> {
    let mut encoder = png::Encoder::new(output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    match mode {
        "none" => encoder.set_compression(png::Compression::NoCompression),
        "fast" => encoder.set_compression(png::Compression::Fast),
        "default" => {}
        _ => return Err("unknown mode".into()),
    }
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    writer.finish()?;
    Ok(())
}
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}
fn measure(
    mut operation: impl FnMut() -> Result<(), AnyError>,
    count: usize,
) -> Result<f64, AnyError> {
    let start = Instant::now();
    for _ in 0..count {
        operation()?;
    }
    Ok(start.elapsed().as_secs_f64() * 1_000_000.0 / count as f64)
}
fn main() -> Result<(), AnyError> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).ok_or("mode required")?;
    if mode == "memory-ours" {
        let input = std::fs::read(&args[2])?;
        let decoded = raipixel::png::decode(&input, Limits::default(), &mut Work::default())?;
        black_box(decoded);
        return Ok(());
    }
    if mode == "memory-crate" {
        let input = std::fs::read(&args[2])?;
        black_box(reference_decode(&input)?);
        return Ok(());
    }
    if mode == "features" {
        let mut sixteen = Vec::new();
        let mut encoder = png::Encoder::new(&mut sixteen, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Sixteen);
        let samples = [0x12, 0x34, 0xab, 0xcd, 0, 0, 0xff, 0xff];
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&samples)?;
        writer.finish()?;
        let mut reader = png::Decoder::new(Cursor::new(&sixteen)).read_info()?;
        let mut out = vec![0; reader.output_buffer_size().ok_or("size")?];
        let info = reader.next_frame(&mut out)?;
        reader.finish()?;
        assert_eq!(info.bit_depth, png::BitDepth::Sixteen);
        assert_eq!(out, samples);
        assert_eq!(
            raipixel::png::decode(&sixteen, Limits::default(), &mut Work::default())
                .unwrap_err()
                .code(),
            "E_UNSUPPORTED_DEPTH"
        );
        let mut apng = Vec::new();
        let mut encoder = png::Encoder::new(&mut apng, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 0)?;
        encoder.set_frame_delay(1, 20)?;
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&[1, 2, 3, 255])?;
        writer.write_image_data(&[4, 5, 6, 255])?;
        writer.finish()?;
        let mut reader = png::Decoder::new(Cursor::new(&apng)).read_info()?;
        assert_eq!(
            reader
                .info()
                .animation_control
                .ok_or("animation metadata")?
                .num_frames,
            2
        );
        let mut out = vec![0; reader.output_buffer_size().ok_or("size")?];
        reader.next_frame(&mut out)?;
        assert_eq!(out, [1, 2, 3, 255]);
        reader.next_frame(&mut out)?;
        assert_eq!(out, [4, 5, 6, 255]);
        reader.finish()?;
        assert_eq!(
            raipixel::png::decode(&apng, Limits::default(), &mut Work::default())
                .unwrap_err()
                .code(),
            "E_UNSUPPORTED_ANIMATION"
        );
        println!(
            "Reference 16-bit and two-frame APNG read/write passed; raipixel explicitly rejects both"
        );
        return Ok(());
    }
    if mode == "verify-ours" || mode == "reencode-ours" || mode == "reencode-compressed" {
        let input = std::fs::read(&args[2])?;
        let decoded = raipixel::png::decode(&input, Limits::default(), &mut Work::default())?;
        let mut out = std::io::stdout().lock();
        if mode == "reencode-compressed" {
            raipixel::png::encode_compressed(&decoded.image, &mut out, &mut Work::default())?;
        } else if mode == "reencode-ours" {
            raipixel::png::encode(&decoded.image, &mut out, &mut Work::default())?;
        } else {
            for dim in decoded.image.size() {
                out.write_all(&dim.to_be_bytes())?;
            }
            out.write_all(decoded.image.pixels().as_flattened())?;
        }
        return Ok(());
    }
    if mode == "verify" {
        let input = std::fs::read(&args[2])?;
        let (w, h, rgba) = reference_decode(&input)?;
        let mut out = std::io::stdout().lock();
        out.write_all(&w.to_be_bytes())?;
        out.write_all(&h.to_be_bytes())?;
        out.write_all(&rgba)?;
        return Ok(());
    }
    eprintln!(
        "png=0.18.1 default_compression={:?}",
        png::Compression::default()
    );
    let input = std::fs::read(&args[2])?;
    let iterations: usize = args[3].parse()?;
    let ours = raipixel::png::decode(&input, Limits::default(), &mut Work::default())?;
    let (w, h, rgba) = reference_decode(&input)?;
    assert_eq!(ours.image.pixels().as_flattened(), rgba);
    let folder = std::path::Path::new(&args[4]);
    std::fs::create_dir_all(folder)?;
    let mut ours_output = Vec::new();
    raipixel::png::encode(&ours.image, &mut ours_output, &mut Work::default())?;
    std::fs::write(folder.join("ours.png"), &ours_output)?;
    let mut sizes = vec![("ours", ours_output.len())];
    for mode in ["none", "fast", "default"] {
        let mut out = Vec::new();
        reference_encode(&mut out, &rgba, w, h, mode)?;
        let decoded = raipixel::png::decode(&out, Limits::default(), &mut Work::default())?;
        assert_eq!(
            decoded.image.pixels().as_flattened(),
            rgba,
            "reference {mode} output"
        );
        let result = reference_decode(&ours_output)?;
        assert_eq!(result.2, rgba);
        sizes.push((mode, out.len()));
        std::fs::write(folder.join(format!("{mode}.png")), out)?;
    }
    let mut compressed = Vec::new();
    raipixel::png::encode_compressed(&ours.image, &mut compressed, &mut Work::default())?;
    assert_eq!(reference_decode(&compressed)?.2, rgba);
    assert_eq!(
        raipixel::png::decode(&compressed, Limits::default(), &mut Work::default())?.image,
        ours.image
    );
    sizes.push(("compressed", compressed.len()));
    std::fs::write(folder.join("compressed.png"), compressed)?;
    let mut timings: Vec<Vec<f64>> = (0..7).map(|_| Vec::new()).collect();
    // Rotate execution order across seven batches; medians reduce shared-host noise.
    for round in 0..7 {
        for n in 0..7 {
            let index = (n + round) % 7;
            let time = match index {
                0 => measure(
                    || {
                        black_box(raipixel::png::decode(
                            black_box(&input),
                            Limits::default(),
                            &mut Work::default(),
                        )?);
                        Ok(())
                    },
                    iterations,
                )?,
                1 => measure(
                    || {
                        black_box(reference_decode(black_box(&input))?);
                        Ok(())
                    },
                    iterations,
                )?,
                2 => measure(
                    || {
                        let mut out = Vec::new();
                        raipixel::png::encode(
                            black_box(&ours.image),
                            &mut out,
                            &mut Work::default(),
                        )?;
                        black_box(out);
                        Ok(())
                    },
                    iterations,
                )?,
                3 => measure(
                    || {
                        let mut out = Vec::new();
                        reference_encode(&mut out, black_box(&rgba), w, h, "none")?;
                        black_box(out);
                        Ok(())
                    },
                    iterations,
                )?,
                4 => measure(
                    || {
                        let mut out = Vec::new();
                        reference_encode(&mut out, black_box(&rgba), w, h, "fast")?;
                        black_box(out);
                        Ok(())
                    },
                    iterations,
                )?,
                5 => measure(
                    || {
                        let mut out = Vec::new();
                        reference_encode(&mut out, black_box(&rgba), w, h, "default")?;
                        black_box(out);
                        Ok(())
                    },
                    iterations,
                )?,
                6 => measure(
                    || {
                        let mut out = Vec::new();
                        raipixel::png::encode_compressed(
                            black_box(&ours.image),
                            &mut out,
                            &mut Work::default(),
                        )?;
                        black_box(out);
                        Ok(())
                    },
                    iterations,
                )?,
                _ => unreachable!(),
            };
            timings[index].push(time);
        }
    }
    print!(
        "{{\"width\":{w},\"height\":{h},\"input_bytes\":{},\"iterations\":{iterations},\"us\":[",
        input.len()
    );
    for (i, times) in timings.into_iter().enumerate() {
        if i != 0 {
            print!(",");
        }
        print!("{:.4}", median(times));
    }
    print!("],\"bytes\":{{");
    for (i, (name, bytes)) in sizes.into_iter().enumerate() {
        if i != 0 {
            print!(",");
        }
        print!("\"{name}\":{bytes}");
    }
    println!("}}}}");
    Ok(())
}
