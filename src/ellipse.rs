//! Exact center-sampled raster ellipses and their four-neighbor outlines.
use crate::{Error, Image, Pixel, Rect, Work};

/// Test a pixel center against the integer ellipse inequality.
///
/// The work-budget preflight in `ellipse` bounds the bounding-box area to
/// at most one billion before evaluating any squared products in i128.
fn inside(x: i64, y: i64, bounds: [i64; 4], squared: [i128; 3]) -> bool {
    if x < bounds[0] || x >= bounds[2] || y < bounds[1] || y >= bounds[3] {
        return false;
    }
    let dx = i128::from(2 * x + 1 - bounds[0] - bounds[2]);
    let dy = i128::from(2 * y + 1 - bounds[1] - bounds[3]);
    dx * dx * squared[1] + dy * dy * squared[0] <= squared[2]
}

impl Image {
    /// Draw a filled ellipse or its exact four-neighbor perimeter.
    ///
    /// Every pixel is selected by its cell center using integer arithmetic;
    /// outline pixels are inside pixels with an outside cardinal neighbor.
    /// The declared bounding-box area is charged before mutation, including
    /// clipped candidates. Neither path allocates or changes hidden RGB in
    /// pixels outside the shape. Returns clipped bounding-box candidates.
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
        let w = i128::from(bounds[2] - bounds[0]);
        let h = i128::from(bounds[3] - bounds[1]);
        let squared = [w * w, h * h, w * w * h * h];
        for y in top..bottom {
            let py = i64::from(y);
            for x in left..right {
                let px = i64::from(x);
                if !inside(px, py, bounds, squared) {
                    continue;
                }
                if !filled
                    && inside(px - 1, py, bounds, squared)
                    && inside(px + 1, py, bounds, squared)
                    && inside(px, py - 1, bounds, squared)
                    && inside(px, py + 1, bounds, squared)
                {
                    continue;
                }
                let index = self.index(x, y);
                self.pixels_mut()[index] = color;
            }
        }
        Ok(rect.area() - u64::from(right - left) * u64::from(bottom - top))
    }
}
