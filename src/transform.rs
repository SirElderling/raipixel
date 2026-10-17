//! Exact whole-cel pixel permutations without allocations or resampling.
use crate::{Error, Image, Work};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transform {
    FlipX,
    FlipY,
    Rotate180,
}

impl Image {
    /// Permute the complete image in place. Every RGBA byte, including hidden
    /// RGB at alpha zero, is retained. A failed work charge makes no changes.
    /// The dimensions and image allocation are unchanged.
    pub fn transform(&mut self, kind: Transform, work: &mut Work) -> Result<(), Error> {
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
        }
        Ok(())
    }
}
