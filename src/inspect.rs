//! Exact image inspection with bounded scratch and deterministic histogram ties.
//! Pixel values include meaningful RGB even when alpha is zero.
use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::{Error, Image, Pixel, Rect, Resource, Work};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorCount {
    pub rgba: Pixel,
    pub count: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ImageStats {
    pub size: [u32; 2],
    /// Absolute image-local region [x, y, width, height].
    pub region: [u32; 4],
    pub pixel_count: u64,
    pub zero_alpha: u64,
    pub partial_alpha: u64,
    pub opaque_alpha: u64,
    pub visible_bbox: Option<[u32; 4]>,
    pub unique_colors: u64,
    pub histogram: Vec<ColorCount>,
    pub histogram_truncated: bool,
    /// Optional row-major original bytes, limited to 4096 requested pixels.
    pub pixels: Option<Vec<Pixel>>,
}

// A max-heap whose top element is the *worst* retained histogram entry.
// Higher count and lexicographically smaller RGBA are better.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Worst(ColorCount);
impl Ord for Worst {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .count
            .cmp(&self.0.count)
            .then_with(|| self.0.rgba.cmp(&other.0.rgba))
    }
}
impl PartialOrd for Worst {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Image {
    /// Calculate exact RGBA statistics without changing pixels.
    ///
    /// Scratch for histogram computation is one RGBA sample per inspected
    /// pixel, plus at most 256 heap entries. An optional raw-pixel response is
    /// limited to 4096 samples. The original image is never modified.
    pub fn inspect(
        &self,
        region: Option<Rect>,
        histogram_limit: usize,
        include_pixels: bool,
        work: &mut Work,
    ) -> Result<ImageStats, Error> {
        if !(1..=256).contains(&histogram_limit) {
            return Err(Error::PixelCount);
        }
        let [x0, y0, x1, y1] = match region {
            Some(rect) => self.coverage(rect, false)?,
            None => [0, 0, self.width, self.height],
        };
        let width = x1 - x0;
        let height = y1 - y0;
        let count = u64::from(width) * u64::from(height);
        if include_pixels && count > 4096 {
            return Err(Error::Limit(Resource::Pixels));
        }
        // Charge bounded sorting and copying before allocation or traversal.
        work.charge(count.saturating_mul(32))?;
        let mut sorted = Vec::new();
        sorted
            .try_reserve_exact(count as usize)
            .map_err(|_| Error::Allocation)?;
        let mut original = if include_pixels {
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(count as usize)
                .map_err(|_| Error::Allocation)?;
            Some(pixels)
        } else {
            None
        };
        let mut zero_alpha = 0u64;
        let mut partial_alpha = 0u64;
        let mut opaque_alpha = 0u64;
        let (mut min_x, mut min_y) = (x1, y1);
        let (mut max_x, mut max_y) = (x0, y0);

        for y in y0..y1 {
            let row = self.index(x0, y);
            for (offset, &pixel) in self.pixels()[row..row + width as usize]
                .iter()
                .enumerate()
            {
                sorted.push(pixel);
                if let Some(out) = &mut original {
                    out.push(pixel);
                }
                match pixel[3] {
                    0 => zero_alpha += 1,
                    255 => {
                        opaque_alpha += 1;
                        let x = x0 + offset as u32;
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                        min_y = min_y.min(y);
                        max_y = max_y.max(y);
                    }
                    _ => {
                        partial_alpha += 1;
                        let x = x0 + offset as u32;
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                        min_y = min_y.min(y);
                        max_y = max_y.max(y);
                    }
                }
            }
        }
        let bbox = if partial_alpha + opaque_alpha == 0 {
            None
        } else {
            Some([min_x, min_y, max_x - min_x + 1, max_y - min_y + 1])
        };
        sorted.sort_unstable();
        let mut best = BinaryHeap::new();
        best.try_reserve(histogram_limit)
            .map_err(|_| Error::Allocation)?;
        let mut distinct = 0u64;
        let mut offset = 0;
        while offset < sorted.len() {
            let color = sorted[offset];
            let first = offset;
            offset += 1;
            while offset < sorted.len() && sorted[offset] == color {
                offset += 1;
            }
            distinct += 1;
            let item = Worst(ColorCount {
                rgba: color,
                count: (offset - first) as u64,
            });
            if best.len() < histogram_limit {
                best.push(item);
            } else if let Some(worst) = best.peek()
                && item < *worst
            {
                best.pop();
                best.push(item);
            }
        }
        let mut histogram: Vec<_> = best.into_iter().map(|item| item.0).collect();
        histogram.sort_unstable_by(|a, b| {
            b.count.cmp(&a.count).then_with(|| a.rgba.cmp(&b.rgba))
        });
        Ok(ImageStats {
            size: self.size(),
            region: [x0, y0, width, height],
            pixel_count: count,
            zero_alpha,
            partial_alpha,
            opaque_alpha,
            visible_bbox: bbox,
            unique_colors: distinct,
            histogram,
            histogram_truncated: distinct > histogram_limit as u64,
            pixels: original,
        })
    }
}
