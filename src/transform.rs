//! Exact whole-cel pixel permutations without allocations or resampling.
use crate::{Error, Image, Work};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transform {
    FlipX,
    FlipY,
    Rotate180,
    Rotate90,
    Rotate270,
}

impl Image {
    /// Permute the complete image in place. Every RGBA byte, including hidden
    /// RGB at alpha zero, is retained. A failed work charge makes no changes.
    /// The dimensions and image allocation are unchanged.
    pub fn transform(&mut self, kind: Transform, work: &mut Work) -> Result<(), Error> {
        if matches!(kind, Transform::Rotate90 | Transform::Rotate270) {
            // Destination-row traversal writes sequentially into one new RGBA
            // allocation. Source cels remain unchanged on limit/OOM failures.
            let mut rotated = Image::new(self.height, self.width, self.limits, work)?;
            let source = self.pixels();
            let original_width = self.width as usize;
            let original_height = self.height as usize;
            for (y, row) in rotated
                .pixels_mut()
                .chunks_exact_mut(original_height)
                .enumerate()
            {
                for (x, destination) in row.iter_mut().enumerate() {
                    let original_x = if matches!(kind, Transform::Rotate90) {
                        y
                    } else {
                        original_width - 1 - y
                    };
                    let original_y = if matches!(kind, Transform::Rotate90) {
                        original_height - 1 - x
                    } else {
                        x
                    };
                    *destination = source[original_y * original_width + original_x];
                }
            }
            *self = rotated;
            return Ok(());
        }
        work.charge(self.pixels().len() as u64)?;
        let width = self.width as usize;
        let height = self.height as usize;
        let pixels = self.pixels_mut();
        match kind {
            Transform::FlipX => {
                for row in pixels.chunks_exact_mut(width) {
                    row.reverse();
                }
            }
            Transform::FlipY => {
                for y in 0..height / 2 {
                    let top = y * width;
                    let bottom = (height - 1 - y) * width;
                    for x in 0..width {
                        pixels.swap(top + x, bottom + x);
                    }
                }
            }
            Transform::Rotate180 => pixels.reverse(),
            Transform::Rotate90 | Transform::Rotate270 => unreachable!("handled above"),
        }
        Ok(())
    }
}
