//! Deterministic extraction of exact visible RGB palettes from stored RGBA pixels.
use crate::{Error, Image, Resource, Work};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteColor {
    pub rgb: [u8; 3],
    pub count: u64,
}

impl Image {
    /// Return exact visible RGB colors in frequency-descending, RGB-ascending
    /// order. Pixel alpha must be nonzero to contribute; hidden RGB is intact.
    ///
    /// Reject empty palettes or colors beyond the requested 1..=256 limit,
    /// reporting the actual distinct count without quantization/truncation.
    /// Sorting a bounded scratch image is preferred over a custom unbounded
    /// hash map so adversarial inputs have deterministic work and memory.
    pub fn extract_visible_palette(
        &self,
        max_colors: usize,
        work: &mut Work,
    ) -> Result<Vec<PaletteColor>, Error> {
        if !(1..=256).contains(&max_colors) {
            return Err(Error::PixelCount);
        }
        let count = self.pixels().len() as u64;
        // The live RGBA allocation plus one three-byte RGB key per pixel.
        let logical_bytes = count
            .checked_mul(7)
            .ok_or(Error::Limit(Resource::Bytes))?;
        self.limits.check_buffer(logical_bytes)?;
        work.charge(count.saturating_mul(32))?;
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
        palette.sort_unstable_by(|a, b| {
            b.count.cmp(&a.count).then_with(|| a.rgb.cmp(&b.rgb))
        });
        Ok(palette)
    }
}
