//! Dependency-free full-codec timings. See benchmarks/png/timing.md.
use raipixel::{Image, Limits, Work, png};
use std::{
    hint::black_box,
    path::Path,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const ROUNDS: usize = 7;

struct Case {
    name: String,
    image: Image,
    input: Vec<u8>,
    stored: Vec<u8>,
    compressed: Vec<u8>,
}
impl Case {
    fn new(name: String, image: Image, input: Option<Vec<u8>>) -> Result<Self> {
        let mut stored = Vec::new();
        let mut compressed = Vec::new();
        png::encode(&image, &mut stored, &mut Work::default())?;
        png::encode_compressed(&image, &mut compressed, &mut Work::default())?;
        for bytes in [&stored, &compressed] {
            assert_eq!(
                png::decode(bytes, Limits::default(), &mut Work::default())?.image,
                image
            );
        }
        let input = input.unwrap_or_else(|| compressed.clone());
        Ok(Self {
            name,
            image,
            input,
            stored,
            compressed,
        })
    }
    fn operation(&self, mode: usize) -> Result<()> {
        match mode {
            0 | 1 => {
                let input = if mode == 0 { &self.input } else { &self.stored };
                black_box(png::decode(
                    black_box(input),
                    Limits::default(),
                    &mut Work::default(),
                )?);
            }
            2 | 3 => {
                let mut output = Vec::new();
                if mode == 2 {
                    png::encode(black_box(&self.image), &mut output, &mut Work::default())?;
                } else {
                    png::encode_compressed(
                        black_box(&self.image),
                        &mut output,
                        &mut Work::default(),
                    )?;
                }
                black_box(output);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    fn batch(&self, mode: usize, count: usize) -> Result<Duration> {
        let start = Instant::now();
        for _ in 0..count {
            self.operation(mode)?;
        }
        Ok(start.elapsed())
    }
    fn measure(&self, minimum: Duration) -> Result<()> {
        let mut counts = [1; 4];
        // Calibration also warms code, input pages and allocator. Independent
        // counts avoid timing tiny calls for only a handful of clock ticks.
        for (mode, count) in counts.iter_mut().enumerate() {
            while self.batch(mode, *count)? < minimum {
                *count = count.checked_mul(2).ok_or("iteration overflow")?;
            }
        }
        let mut rounds = [[0.0; 4]; ROUNDS];
        for (round, samples) in rounds.iter_mut().enumerate() {
            for offset in 0..4 {
                let mode = (round + offset) % 4;
                samples[mode] = self.batch(mode, counts[mode])?.as_secs_f64() * 1_000_000.0
                    / counts[mode] as f64;
            }
        }
        let samples: [[f64; ROUNDS]; 4] =
            std::array::from_fn(|mode| std::array::from_fn(|round| rounds[round][mode]));
        // JSON escaping through our existing emitter; all reporting is untimed.
        let name = raipixel::json::emit(
            &raipixel::json::Value::String(self.name.clone()),
            raipixel::json::Limits::default(),
            &mut Work::default(),
        )?;
        println!(
            "{{\"case\":{},\"width\":{},\"height\":{},\"input_bytes\":{},\"stored_bytes\":{},\"compressed_bytes\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",
            std::str::from_utf8(&name)?.trim(),
            self.image.size()[0],
            self.image.size()[1],
            self.input.len(),
            self.stored.len(),
            self.compressed.len(),
            counts,
            samples
        );
        Ok(())
    }
}

fn suite() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for (name, size) in [
        ("icon", 16),
        ("icon", 64),
        ("sheet", 1024),
        ("texture", 1024),
        ("noise", 256),
    ] {
        let mut state = 0x1234_5678_u32;
        let mut pixels = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let pixel = match name {
                    "texture" => [
                        (x * 19 + y * 7) as u8,
                        (x * 37 + y * 11) as u8,
                        (x * 71 + y * 13) as u8,
                        if (x + y) % 7 == 0 { 0 } else { 255 },
                    ],
                    "noise" => {
                        state ^= state << 13;
                        state ^= state >> 17;
                        state ^= state << 5;
                        state.to_le_bytes()
                    }
                    _ => {
                        let scale = if size == 16 { 1 } else { 4 };
                        let (x, y) = ((x / scale) % 16, (y / scale) % 16);
                        if (6..=9).contains(&x) && y < 10 {
                            [173, 184, 198, 255]
                        } else if (4..=11).contains(&x) && y == 10 {
                            [197, 145, 42, 255]
                        } else if (7..=8).contains(&x) && y > 10 {
                            [98, 51, 26, 255]
                        } else {
                            [17, 23, 31, 0]
                        }
                    }
                };
                pixels.push(pixel);
            }
        }
        let image = Image::from_pixels(
            size as u32,
            size as u32,
            &pixels,
            Limits::default(),
            &mut Work::default(),
        )?;
        cases.push(Case::new(format!("{name}-{size}"), image, None)?);
    }
    Ok(cases)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let source = args
        .first()
        .ok_or("usage: png_bench INPUT.png|--suite|--export [MIN_BATCH_MS|DIRECTORY]")?;
    let cases = if source.starts_with("--") {
        if source != "--suite" && source != "--export" {
            return Err("unknown mode".into());
        }
        suite()?
    } else {
        let input = std::fs::read(source)?;
        let image = png::decode(&input, Limits::default(), &mut Work::default())?.image;
        vec![Case::new(source.clone(), image, Some(input))?]
    };
    if source == "--export" {
        let path = Path::new(args.get(1).ok_or("missing export directory")?);
        std::fs::create_dir_all(path)?;
        for case in cases {
            std::fs::write(path.join(format!("{}-stored.png", case.name)), case.stored)?;
            std::fs::write(
                path.join(format!("{}-compressed.png", case.name)),
                case.compressed,
            )?;
        }
    } else {
        let milliseconds: u64 = args.get(1).map_or(Ok(25), |s| s.parse())?;
        if milliseconds == 0 {
            return Err("batch duration must be positive".into());
        }
        for case in cases {
            case.measure(Duration::from_millis(milliseconds))?;
        }
    }
    Ok(())
}
