//! Integer nearest-neighbor scaling with exact RGBA samples and bounded work.
use crate::{Error, Image, Resource, Work};

impl Image {
    /// Repeat source pixels horizontally and rows vertically without resampling.
    ///
    /// Allocates one output image, charges one work unit per output pixel,
    /// preserves all four RGBA bytes (including hidden transparent RGB), and
    /// leaves the source unchanged on every success or failure.
    pub fn scaled_nearest(&self, factor: u32, work: &mut Work) -> Result<Image, Error> {
        if factor == 0 {
            return Err(Error::Dimensions);
        }
        let width = self
            .width
            .checked_mul(factor)
            .ok_or(Error::Limit(Resource::Side))?;
        let height = self
            .height
            .checked_mul(factor)
            .ok_or(Error::Limit(Resource::Side))?;
        let mut output = Image::new(width, height, self.limits, work)?;
        let source = self.pixels();
        let destination = output.pixels_mut();
        let original_width = self.width as usize;
        let repeat = factor as usize;
        let expanded_width = width as usize;
        for y in 0..self.height as usize {
            let start = y * repeat * expanded_width;
            for (x, &color) in source[y * original_width..(y + 1) * original_width]
                .iter()
                .enumerate()
            {
                let target = start + x * repeat;
                destination[target..target + repeat].fill(color);
            }
            for row in 1..repeat {
                destination
                    .copy_within(start..start + expanded_width, start + row * expanded_width);
            }
        }
        Ok(output)
    }
}
