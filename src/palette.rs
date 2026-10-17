//! Deterministic extraction of exact visible RGB palettes from stored RGBA pixels.
use crate::{Error, Image, Resource, Work};

const COUNT_SLOTS: usize = 2048;
const MAX_PROBES: usize = 32;
// Small sprites should not pay for a 48 KiB counter table. Tested crossover
// on Linux x86-64, macOS ARM64 and Windows MSVC before selecting 64x64.
const COUNT_MIN_PIXELS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteColor {
    pub rgb: [u8; 3],
    pub count: u64,
}

#[inline]
fn slot(rgb: [u8; 3]) -> usize {
    let key = u32::from_be_bytes([0, rgb[0], rgb[1], rgb[2]]);
    (key.wrapping_mul(0x9E37_79B1) as usize) & (COUNT_SLOTS - 1)
}

impl Image {
    /// Return exact visible RGB colors ordered by descending frequency, then RGB.
    ///
    /// Transparent RGB is not counted or modified. Small images use bounded RGB
    /// sorting; larger images use a fixed-capacity counter with deterministic
    /// worst-case sorting fallback for unusual collisions or too many colors.
    /// Both methods share the original maximum, error and work semantics.
    pub fn extract_visible_palette(
        &self,
        max_colors: usize,
        work: &mut Work,
    ) -> Result<Vec<PaletteColor>, Error> {
        if !(1..=256).contains(&max_colors) {
            return Err(Error::PixelCount);
        }
        let count = self.pixels().len() as u64;
        let sort_bytes = count.checked_mul(7).ok_or(Error::Limit(Resource::Bytes))?;
        let counter_bytes = count
            .checked_mul(4)
            .and_then(|b| {
                b.checked_add(
                    (COUNT_SLOTS * std::mem::size_of::<Option<PaletteColor>>()) as u64
                        + (max_colors * std::mem::size_of::<PaletteColor>()) as u64,
                )
            })
            .ok_or(Error::Limit(Resource::Bytes))?;
        let use_counter = self.pixels().len() >= COUNT_MIN_PIXELS
            && self.limits.check_buffer(counter_bytes).is_ok();
        if !use_counter {
            // Limited-memory callers that previously fit the sort scratch
            // never regress merely because the fixed counter table is larger.
            self.limits.check_buffer(sort_bytes)?;
        }
        work.charge(count.saturating_mul(32))?;
        if use_counter {
            if let Some(palette) = self.extract_counted(max_colors)? {
                return Ok(palette);
            }
            // For >max_colors and adversarial hash chains, sorting still
            // determines the exact number of distinct RGB values.
            self.limits.check_buffer(sort_bytes)?;
        }
        self.extract_sorted(max_colors)
    }

    fn extract_counted(&self, max_colors: usize) -> Result<Option<Vec<PaletteColor>>, Error> {
        let mut slots = [None::<PaletteColor>; COUNT_SLOTS];
        let mut distinct = 0;
        for &pixel in self.pixels() {
            if pixel[3] == 0 {
                continue;
            }
            let rgb = [pixel[0], pixel[1], pixel[2]];
            let mut at = slot(rgb);
            let mut found = false;
            for _ in 0..MAX_PROBES {
                match &mut slots[at] {
                    Some(entry) if entry.rgb == rgb => {
                        entry.count += 1;
                        found = true;
                        break;
                    }
                    None if distinct < max_colors => {
                        slots[at] = Some(PaletteColor { rgb, count: 1 });
                        distinct += 1;
                        found = true;
                        break;
                    }
                    None => return Ok(None),
                    _ => at = (at + 1) & (COUNT_SLOTS - 1),
                }
            }
            if !found {
                return Ok(None);
            }
        }
        if distinct == 0 {
            return Err(Error::PaletteEmpty);
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(distinct)
            .map_err(|_| Error::Allocation)?;
        result.extend(slots.into_iter().flatten());
        result.sort_unstable_by(|a, b| b.count.cmp(&a.count).then_with(|| a.rgb.cmp(&b.rgb)));
        Ok(Some(result))
    }

    fn extract_sorted(&self, max_colors: usize) -> Result<Vec<PaletteColor>, Error> {
        let mut pixels = Vec::<[u8; 3]>::new();
        pixels
            .try_reserve_exact(self.pixels().len())
            .map_err(|_| Error::Allocation)?;
        for &pixel in self.pixels() {
            if pixel[3] != 0 {
                pixels.push([pixel[0], pixel[1], pixel[2]]);
            }
        }
        if pixels.is_empty() {
            return Err(Error::PaletteEmpty);
        }
        pixels.sort_unstable();
        let mut palette = Vec::new();
        palette
            .try_reserve_exact(max_colors)
            .map_err(|_| Error::Allocation)?;
        let mut distinct = 0u32;
        let mut index = 0;
        while index < pixels.len() {
            let rgb = pixels[index];
            let from = index;
            index += 1;
            while index < pixels.len() && pixels[index] == rgb {
                index += 1;
            }
            distinct += 1;
            if (distinct as usize) <= max_colors {
                palette.push(PaletteColor {
                    rgb,
                    count: (index - from) as u64,
                });
            }
        }
        if (distinct as usize) > max_colors {
            return Err(Error::PaletteTooMany {
                limit: max_colors as u16,
                distinct,
            });
        }
        palette.sort_unstable_by(|a, b| b.count.cmp(&a.count).then_with(|| a.rgb.cmp(&b.rgb)));
        Ok(palette)
    }
}
