//! Two complete visible-palette extraction strategies with exact output parity.
//! Run: cargo run --release --locked --offline --example bench_palette_extract
//! Candidate B is EXPERIMENTAL: never used for real jobs without a later
//! resource-accounting and adversarial-input acceptance decision.

use raipixel::{Error, Image, Limits, Work, palette::PaletteColor};
use std::{
    hint::black_box,
    mem::size_of,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

const SLOTS: usize = 2048;
const MAX_PROBES: usize = 32;
static FALLBACKS: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn bucket(rgb: [u8; 3]) -> usize {
    let packed = u32::from_be_bytes([0, rgb[0], rgb[1], rgb[2]]);
    (packed.wrapping_mul(0x9E37_79B1) as usize) & (SLOTS - 1)
}

// B: one bounded stack table for <=256 visible colors. Source and output
// semantics agree with A; pathological collisions or >256 colors explicitly
// revert to A instead of dropping counts or silently truncating colors.
// This benchmark does not claim bounded aggregate process RSS.
fn counted(image: &Image, work: &mut Work) -> Result<Vec<PaletteColor>, Error> {
    let mut slots = [None::<PaletteColor>; SLOTS];
    let mut distinct = 0usize;
    for &rgba in image.pixels() {
        if rgba[3] == 0 {
            continue;
        }
        let rgb = [rgba[0], rgba[1], rgba[2]];
        let mut at = bucket(rgb);
        let mut done = false;
        for _ in 0..MAX_PROBES {
            match &mut slots[at] {
                Some(entry) if entry.rgb == rgb => {
                    entry.count += 1;
                    done = true;
                    break;
                }
                None if distinct < 256 => {
                    slots[at] = Some(PaletteColor { rgb, count: 1 });
                    distinct += 1;
                    done = true;
                    break;
                }
                None => break,
                _ => at = (at + 1) & (SLOTS - 1),
            }
        }
        if !done {
            FALLBACKS.fetch_add(1, Ordering::Relaxed);
            return image.extract_visible_palette(256, work);
        }
    }
    // Match the baseline's charged work for the accepted 1..256-color profile.
    work.charge((image.pixels().len() as u64).saturating_mul(32))?;
    if distinct == 0 {
        return Err(Error::PaletteEmpty);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(distinct)
        .map_err(|_| Error::Allocation)?;
    result.extend(slots.into_iter().flatten());
    result.sort_unstable_by(|a, b| b.count.cmp(&a.count).then_with(|| a.rgb.cmp(&b.rgb)));
    Ok(result)
}

// Frozen algorithm A: whole-image RGB scratch + unstable sort, the former
// production implementation. Do not substitute the currently selected public
// API here: otherwise this A/B benchmark would compare B against itself.
fn sorted(image: &Image, work: &mut Work) -> Result<Vec<PaletteColor>, Error> {
    let count = image.pixels().len() as u64;
    work.charge(count.saturating_mul(32))?;
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(image.pixels().len())
        .map_err(|_| Error::Allocation)?;
    for &sample in image.pixels() {
        if sample[3] != 0 {
            rgb.push([sample[0], sample[1], sample[2]]);
        }
    }
    if rgb.is_empty() {
        return Err(Error::PaletteEmpty);
    }
    rgb.sort_unstable();
    let mut out = Vec::new();
    out.try_reserve_exact(256).map_err(|_| Error::Allocation)?;
    let mut index = 0;
    let mut distinct = 0u32;
    while index < rgb.len() {
        let sample = rgb[index];
        let from = index;
        index += 1;
        while index < rgb.len() && rgb[index] == sample {
            index += 1;
        }
        distinct += 1;
        if distinct <= 256 {
            out.push(PaletteColor { rgb: sample, count: (index - from) as u64 });
        }
    }
    if distinct > 256 {
        return Err(Error::PaletteTooMany { limit: 256, distinct });
    }
    out.sort_unstable_by(|a, b| b.count.cmp(&a.count).then_with(|| a.rgb.cmp(&b.rgb)));
    Ok(out)
}

fn make_image(side: u32, unique: usize) -> Image {
    let mut pixels = Vec::with_capacity((side as usize) * (side as usize));
    for index in 0..(side as usize * side as usize) {
        let k = ((index / 17) + (index / side as usize / 7)) % unique;
        let k = k as u8;
        let rgba = [
            k.wrapping_mul(73).wrapping_add(11),
            k.wrapping_mul(151).wrapping_add(29),
            k.wrapping_mul(199).wrapping_add(47),
            if index % 19 == 0 { 0 } else { 255 },
        ];
        pixels.push(rgba);
    }
    Image::from_pixels(side, side, &pixels, Limits::default(), &mut Work::default()).unwrap()
}

fn measure(mut f: impl FnMut(), iterations: usize) -> u128 {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed().as_nanos() / iterations as u128
}

fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn run_case(name: &str, side: u32, colors: usize, iterations: usize) {
    let image = make_image(side, colors);
    let mut a_work = Work::default();
    let mut b_work = Work::default();
    let expected = sorted(&image, &mut a_work);
    let actual = counted(&image, &mut b_work);
    assert_eq!(expected, actual, "{name}: output mismatch");
    assert_eq!(
        a_work.remaining(),
        b_work.remaining(),
        "{name}: work mismatch"
    );
    assert_eq!(
        FALLBACKS.load(Ordering::Relaxed),
        0,
        "{name}: unexpected fallback"
    );
    let mut a_times = Vec::new();
    let mut b_times = Vec::new();
    for round in 0..7 {
        let mut a = || {
            black_box(
                image
                    .extract_visible_palette(256, &mut Work::default())
                    .unwrap(),
            );
        };
        let mut b = || {
            black_box(counted(&image, &mut Work::default()).unwrap());
        };
        if round % 2 == 0 {
            a_times.push(measure(&mut a, iterations));
            b_times.push(measure(&mut b, iterations));
        } else {
            b_times.push(measure(&mut b, iterations));
            a_times.push(measure(&mut a, iterations));
        }
    }
    let a = median(a_times);
    let b = median(b_times);
    let ratio = b as f64 / a as f64;
    let n = image.pixels().len();
    let a_scratch = n * size_of::<[u8; 3]>();
    let b_scratch = SLOTS * size_of::<Option<PaletteColor>>();
    println!(
        "{name} pixels={n} colors={colors} sort_ns={a} count_ns={b} count/sort={ratio:.3} \
         sort_rgb_scratch_bytes={a_scratch} count_table_bytes={b_scratch} \
         note=logical_scratch_not_peak_RSS"
    );
}

fn main() {
    println!(
        "comparison=visible_palette platform={}-{} profile=release rounds=7 \
         sort=accepted count=experimental",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    run_case("tiny_4", 16, 4, 1000);
    run_case("small_4", 32, 4, 450);
    run_case("small_16", 32, 16, 450);
    run_case("middle_16", 48, 16, 250);
    run_case("icon_16", 64, 16, 150);
    run_case("sheet_16", 1024, 16, 8);
    run_case("sheet_256", 1024, 256, 8);

    let blank = Image::new(16, 16, Limits::default(), &mut Work::default()).unwrap();
    assert_eq!(
        sorted(&blank, &mut Work::default()),
        counted(&blank, &mut Work::default()),
        "transparent-only case"
    );
    let mut many = Vec::new();
    for index in 0..257u16 {
        many.push([(index & 255) as u8, (index >> 8) as u8, 77, 255]);
    }
    let overflow =
        Image::from_pixels(257, 1, &many, Limits::default(), &mut Work::default()).unwrap();
    assert_eq!(
        sorted(&overflow, &mut Work::default()),
        counted(&overflow, &mut Work::default()),
        "overflow must report the exact distinct count"
    );
    assert_eq!(FALLBACKS.load(Ordering::Relaxed), 1);
    println!("parity=pass empty=pass overflow=pass fallback_count=1");
}
