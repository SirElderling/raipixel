//! Exact original-sample comparison for AI correction and revision inspection.
//! Transparent RGB remains semantically meaningful; no changes are inferred.

use crate::{Error, Image, Rect, Work};

/// Image-local comparison counts and the smallest half-open changed rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDiff {
    pub region: [u32; 4],
    pub changed_samples: u64,
    /// Changed visible RGB, including pixels becoming visible or invisible.
    /// RGB changes while both alpha values are zero do not count here.
    pub changed_visible_colors: u64,
    pub changed_alpha: u64,
    pub changed_bbox: Option<[u32; 4]>,
}

impl Image {
    /// Compare all four bytes at every sample without allocating scratch.
    ///
    /// Different canvas dimensions or out-of-bounds regions fail before work
    /// is spent. Every selected pixel charges one work unit, even on equality.
    pub fn compare(
        &self,
        other: &Image,
        region: Option<Rect>,
        work: &mut Work,
    ) -> Result<ImageDiff, Error> {
        if self.size() != other.size() {
            return Err(Error::Dimensions);
        }
        let [x0, y0, x1, y1] = match region {
            Some(rect) => self.coverage(rect, false)?,
            None => [0, 0, self.width, self.height],
        };
        let width = x1 - x0;
        let height = y1 - y0;
        work.charge(u64::from(width) * u64::from(height))?;

        let mut changed_samples = 0u64;
        let mut changed_visible_colors = 0u64;
        let mut changed_alpha = 0u64;
        let mut minimum = [x1, y1];
        let mut maximum = [x0, y0];
        for y in y0..y1 {
            let offset = self.index(x0, y);
            let first = &self.pixels()[offset..offset + width as usize];
            let second = &other.pixels()[offset..offset + width as usize];
            for (column, (&a, &b)) in first.iter().zip(second).enumerate() {
                if a == b {
                    continue;
                }
                changed_samples += 1;
                if a[3] != b[3] {
                    changed_alpha += 1;
                }
                if (a[3] == 0) != (b[3] == 0)
                    || ((a[3] != 0 || b[3] != 0) && a[..3] != b[..3])
                {
                    changed_visible_colors += 1;
                }
                let x = x0 + column as u32;
                minimum[0] = minimum[0].min(x);
                minimum[1] = minimum[1].min(y);
                maximum[0] = maximum[0].max(x);
                maximum[1] = maximum[1].max(y);
            }
        }
        let changed_bbox = if changed_samples == 0 {
            None
        } else {
            Some([
                minimum[0],
                minimum[1],
                maximum[0] - minimum[0] + 1,
                maximum[1] - minimum[1] + 1,
            ])
        };
        Ok(ImageDiff {
            region: [x0, y0, width, height],
            changed_samples,
            changed_visible_colors,
            changed_alpha,
            changed_bbox,
        })
    }
}
