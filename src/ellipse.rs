//! Center-sampled integer ellipses with bounded row-span searches.
use crate::{Error, Image, Pixel, Rect, Work};

/// Exact conic predicate from the functional specification.
/// A successful work preflight bounds the rectangle's area to 1e9, keeping
/// all i128 squares and products within range even for extreme clipped origins.
fn inside(x: i64, y: i64, bounds: [i64; 4], squared: [i128; 3]) -> bool {
    if x < bounds[0] || x >= bounds[2] || y < bounds[1] || y >= bounds[3] {
        return false;
    }
    let dx = i128::from(2 * x + 1 - bounds[0] - bounds[2]);
    let dy = i128::from(2 * y + 1 - bounds[1] - bounds[3]);
    dx * dx * squared[1] + dy * dy * squared[0] <= squared[2]
}

/// Find the inclusive inside span of one row. Pixel-center coverage is
/// symmetric and monotone from each outer edge towards the midpoint.
/// Binary search uses logarithmic expensive conic tests per row.
fn row_span(y: i64, bounds: [i64; 4], squared: [i128; 3]) -> Option<(i64, i64)> {
    let middle = bounds[0] + (bounds[2] - bounds[0] - 1) / 2;
    if !inside(middle, y, bounds, squared) {
        return None;
    }
    let (mut left, mut right) = (bounds[0], middle);
    while left < right {
        let test = left + (right - left) / 2;
        if inside(test, y, bounds, squared) {
            right = test;
        } else {
            left = test + 1;
        }
    }
    Some((left, bounds[0] + bounds[2] - 1 - left))
}

impl Image {
    /// Draw a center-sampled integer ellipse or four-neighbor outline.
    ///
    /// Source geometry uses the exact spec inequality. Once a row's inside
    /// span is found, a filled ellipse writes the entire slice. An outline
    /// compares the current span with its immediate neighbors; only its
    /// exposed cardinal boundary pixels are written. Original RGBA samples
    /// outside the selected shape remain intact, including hidden RGB.
    ///
    /// Charge the full declared rectangle before any mutation; clipping
    /// changes only visible writes, not work. Scratch uses constant memory.
    /// Return the number of discarded bounding-rectangle candidates.
    pub fn ellipse(
        &mut self,
        rect: Rect,
        color: Pixel,
        filled: bool,
        clip: bool,
        work: &mut Work,
    ) -> Result<u64, Error> {
        let [left, top, right, bottom] = self.coverage(rect, clip)?;
        work.charge(rect.area())?;
        let bounds = rect.bounds();
        let width = i128::from(bounds[2] - bounds[0]);
        let height = i128::from(bounds[3] - bounds[1]);
        let squared = [
            width * width,
            height * height,
            width * width * height * height,
        ];
        for y in top..bottom {
            let py = i64::from(y);
            let Some((first, last)) = row_span(py, bounds, squared) else {
                continue;
            };
            let visible_left = first.max(i64::from(left));
            let visible_right = (last + 1).min(i64::from(right));
            if visible_left >= visible_right {
                continue;
            }
            let index = self.index(visible_left as u32, y);
            let end = index + (visible_right - visible_left) as usize;
            let pixels = &mut self.pixels_mut()[index..end];
            if filled {
                pixels.fill(color);
                continue;
            }
            // Evaluate geometric neighbors, not canvas-clipped neighbors,
            // so partial output does not fabricate an edge at a clip border.
            let previous = row_span(py - 1, bounds, squared);
            let following = row_span(py + 1, bounds, squared);
            for (offset, pixel) in pixels.iter_mut().enumerate() {
                let x = visible_left + offset as i64;
                if x == first
                    || x == last
                    || previous.is_none_or(|(a, b)| x < a || x > b)
                    || following.is_none_or(|(a, b)| x < a || x > b)
                {
                    *pixel = color;
                }
            }
        }
        Ok(rect.area() - u64::from(right - left) * u64::from(bottom - top))
    }
}
