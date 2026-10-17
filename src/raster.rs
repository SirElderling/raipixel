use crate::model::reserve;
use crate::{Error, Image, Pixel, Rect, Resource, Work};

#[derive(Debug, Clone, Copy)]
pub enum Blend {
    Copy,
    Over,
}

/// Integer byte-space compositing from design/04-architecture.md.
/// Copy retains RGB and applies opacity only to alpha. Transparent Over is a no-op.
pub fn blend(src: Pixel, dst: Pixel, opacity: u8, mode: Blend) -> Pixel {
    let a = (u32::from(src[3]) * u32::from(opacity) + 127) / 255;
    if matches!(mode, Blend::Copy) {
        return [src[0], src[1], src[2], a as u8];
    }
    if a == 0 {
        return dst;
    }
    let b = u32::from(dst[3]);
    let q = a * 255 + b * (255 - a);
    let mut out = [0; 4];
    for c in 0..3 {
        let numerator = u32::from(src[c]) * a * 255 + u32::from(dst[c]) * b * (255 - a);
        out[c] = ((numerator + q / 2) / q) as u8;
    }
    out[3] = ((q + 127) / 255) as u8;
    out
}

impl Image {

    /// Draw a one-pixel-wide integer Bresenham line with inclusive endpoints.
    ///
    /// Endpoint order cannot affect coverage. Charge the full candidate count
    /// before mutation, including off-canvas samples when clipping is enabled.
    /// A failed bounds/work preflight leaves all pixels unchanged.
    pub fn line(
        &mut self,
        from: [i32; 2],
        to: [i32; 2],
        color: Pixel,
        clip: bool,
        work: &mut Work,
    ) -> Result<u64, Error> {
        let (width, height) = (i64::from(self.width), i64::from(self.height));
        if !clip
            && [from, to].iter().any(|point| {
                i64::from(point[0]) < 0
                    || i64::from(point[1]) < 0
                    || i64::from(point[0]) >= width
                    || i64::from(point[1]) >= height
            })
        {
            return Err(Error::Bounds);
        }

        let (first, last) = if from <= to { (from, to) } else { (to, from) };
        let (mut x, mut y) = (i64::from(first[0]), i64::from(first[1]));
        let (end_x, end_y) = (i64::from(last[0]), i64::from(last[1]));
        let dx = (end_x - x).abs();
        let dy = -(end_y - y).abs();
        let count = dx.max(-dy) as u64 + 1;
        work.charge(count)?;

        let step_x = if x < end_x { 1 } else { -1 };
        let step_y = if y < end_y { 1 } else { -1 };
        let mut error = dx + dy;
        let mut written = 0u64;
        loop {
            if (0..width).contains(&x) && (0..height).contains(&y) {
                let index = self.index(x as u32, y as u32);
                self.pixels_mut()[index] = color;
                written += 1;
            }
            if x == end_x && y == end_y {
                break;
            }
            // Signed i64 arithmetic covers even opposite i32 endpoints.
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += step_x;
            }
            if twice <= dx {
                error += dx;
                y += step_y;
            }
        }
        Ok(count - written)
    }

    /// Fill all four samples. Return the number of discarded pixels.
    /// Preflight failures never modify this image.
    pub fn fill(
        &mut self,
        rect: Rect,
        color: Pixel,
        clip: bool,
        work: &mut Work,
    ) -> Result<u64, Error> {
        let [x0, y0, x1, y1] = self.coverage(rect, clip)?;
        work.charge(rect.area())?;
        for y in y0..y1 {
            let start = self.index(x0, y);
            let end = start + (x1 - x0) as usize;
            self.pixels_mut()[start..end].fill(color);
        }
        Ok(rect.area() - u64::from(x1 - x0) * u64::from(y1 - y0))
    }

    /// Draw the exact canonical integer Bresenham line, including both endpoints.
    ///
    /// Coordinates are ordered lexicographically so reversing the endpoints
    /// cannot change tie-breaking. Work charges the entire declared line before
    /// any mutation, even when clipped. Only in-canvas positions are visited:
    /// extreme off-canvas endpoints never cause an unbounded traversal.
    /// Returns the number of discarded out-of-canvas line pixels.
    pub fn draw_line(
        &mut self,
        start: [i32; 2],
        end: [i32; 2],
        color: Pixel,
        clip: bool,
        work: &mut Work,
    ) -> Result<u64, Error> {
        let (mut a, mut b) = (start, end);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        let (x0, y0) = (i64::from(a[0]), i64::from(a[1]));
        let (x1, y1) = (i64::from(b[0]), i64::from(b[1]));
        let (width, height) = (i64::from(self.width), i64::from(self.height));
        let inside = |x: i64, y: i64| x >= 0 && x < width && y >= 0 && y < height;
        if !clip && (!inside(x0, y0) || !inside(x1, y1)) {
            return Err(Error::Bounds);
        }
        let dx = x1 - x0;
        let dy = (y1 - y0).abs();
        let declared = dx.max(dy) as u64 + 1;
        work.charge(declared)?;
        if dx == 0 && dy == 0 {
            if inside(x0, y0) {
                let index = self.index(x0 as u32, y0 as u32);
                self.pixels_mut()[index] = color;
                return Ok(0);
            }
            return Ok(1);
        }

        let mut written = 0u64;
        if dx >= dy {
            // Closed-form Bresenham minor coordinate with half-up ties.
            // u128 avoids overflow for the complete i32 endpoint domain.
            for x in 0..self.width {
                let x = i64::from(x);
                if x < x0 || x > x1 {
                    continue;
                }
                let rise = ((x - x0) as u128 * dy as u128 + (dx / 2) as u128) / dx as u128;
                let y = y0
                    + if y1 >= y0 {
                        rise as i64
                    } else {
                        -(rise as i64)
                    };
                if inside(x, y) {
                    let index = self.index(x as u32, y as u32);
                    self.pixels_mut()[index] = color;
                    written += 1;
                }
            }
        } else {
            for y in 0..self.height {
                let y = i64::from(y);
                let step = if y1 >= y0 { y - y0 } else { y0 - y };
                if step < 0 || step > dy {
                    continue;
                }
                let rise = (step as u128 * dx as u128 + (dy / 2) as u128) / dy as u128;
                let x = x0 + rise as i64;
                if inside(x, y) {
                    let index = self.index(x as u32, y as u32);
                    self.pixels_mut()[index] = color;
                    written += 1;
                }
            }
        }
        Ok(declared - written)
    }

    /// Draw a compact ASCII color grid, preserving exact hidden RGB samples.
    ///
    /// Each row has the same nonzero byte width. Symbols are printable ASCII;
    /// `None` is an explicit skip, unlike the transparent RGBA pixel [0; 4].
    /// Validation, bounds and the full declared work charge precede all writes.
    /// No temporary pixel grid or heap allocation is required.
    pub fn draw_grid(
        &mut self,
        at: [i32; 2],
        rows: &[&str],
        symbols: &[(u8, Option<Pixel>)],
        clip: bool,
        work: &mut Work,
    ) -> Result<u64, Error> {
        let width = u32::try_from(rows.first().ok_or(Error::Rectangle)?.len())
            .map_err(|_| Error::Rectangle)?;
        let height = u32::try_from(rows.len()).map_err(|_| Error::Rectangle)?;
        let rect = Rect::new(at[0], at[1], width, height)?;
        if symbols.len() > 95 {
            return Err(Error::PixelCount);
        }
        let mut lookup = [None; 95];
        for &(symbol, color) in symbols {
            if !(b' '..=b'~').contains(&symbol) {
                return Err(Error::PixelCount);
            }
            let slot = &mut lookup[usize::from(symbol - b' ')];
            if slot.is_some() {
                return Err(Error::PixelCount);
            }
            *slot = Some(color);
        }
        for row in rows {
            if row.len() != width as usize {
                return Err(Error::PixelCount);
            }
            for &symbol in row.as_bytes() {
                if !(b' '..=b'~').contains(&symbol)
                    || lookup[usize::from(symbol - b' ')].is_none()
                {
                    return Err(Error::PixelCount);
                }
            }
        }
        let coverage = self.coverage(rect, clip)?;
        work.charge(rect.area())?;
        let [x0, y0, x1, y1] = coverage;
        for y in y0..y1 {
            let row = rows[(i64::from(y) - i64::from(at[1])) as usize].as_bytes();
            for x in x0..x1 {
                let column = (i64::from(x) - i64::from(at[0])) as usize;
                if let Some(color) = lookup[usize::from(row[column] - b' ')].unwrap() {
                    let index = self.index(x, y);
                    self.pixels_mut()[index] = color;
                }
            }
        }
        Ok(discarded(rect, coverage))
    }

    /// Stamp an in-bounds source region at an explicit destination position.
    /// Clipping affects only destination coverage, never source validity.
    #[allow(clippy::too_many_arguments)]
    pub fn stamp(
        &mut self,
        source: &Image,
        rect: Rect,
        at: [i32; 2],
        mode: Blend,
        opacity: u8,
        clip: bool,
        work: &mut Work,
    ) -> Result<u64, Error> {
        source.coverage(rect, false)?;
        let target = Rect::new(at[0], at[1], rect.width, rect.height)?;
        let coverage = self.coverage(target, clip)?;
        work.charge(rect.area())?;
        let origin = [rect.x as u32, rect.y as u32];
        self.apply_stamp(
            source.pixels(),
            source.width,
            origin,
            target,
            coverage,
            mode,
            opacity,
        );
        Ok(discarded(target, coverage))
    }

    /// Snapshot the selected source before writing, including overlapping copies.
    /// Work includes both snapshot reads and the full declared stamp area.
    pub fn stamp_within(
        &mut self,
        rect: Rect,
        at: [i32; 2],
        mode: Blend,
        opacity: u8,
        clip: bool,
        work: &mut Work,
    ) -> Result<u64, Error> {
        self.coverage(rect, false)?;
        let target = Rect::new(at[0], at[1], rect.width, rect.height)?;
        let coverage = self.coverage(target, clip)?;
        self.limits.check_bytes(rect.area())?;
        let units = rect
            .area()
            .checked_mul(2)
            .ok_or(Error::Limit(Resource::Work))?;
        work.charge(units)?;
        // Source validity implies area <= this image's checked allocation length.
        let mut snapshot = reserve(rect.area() as usize)?;
        for y in rect.y as u32..rect.y as u32 + rect.height {
            let start = self.index(rect.x as u32, y);
            snapshot.extend_from_slice(&self.pixels()[start..start + rect.width as usize]);
        }
        self.apply_stamp(
            &snapshot,
            rect.width,
            [0, 0],
            target,
            coverage,
            mode,
            opacity,
        );
        Ok(discarded(target, coverage))
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_stamp(
        &mut self,
        samples: &[Pixel],
        stride: u32,
        origin: [u32; 2],
        target: Rect,
        coverage: [u32; 4],
        mode: Blend,
        opacity: u8,
    ) {
        let [x0, y0, x1, y1] = coverage;
        for y in y0..y1 {
            for x in x0..x1 {
                let sx = i64::from(origin[0]) + i64::from(x) - i64::from(target.x);
                let sy = i64::from(origin[1]) + i64::from(y) - i64::from(target.y);
                let src = samples[sy as usize * stride as usize + sx as usize];
                let index = self.index(x, y);
                let pixel = &mut self.pixels_mut()[index];
                *pixel = blend(src, *pixel, opacity, mode);
            }
        }
    }
}

fn discarded(rect: Rect, coverage: [u32; 4]) -> u64 {
    let [x0, y0, x1, y1] = coverage;
    rect.area() - u64::from(x1 - x0) * u64::from(y1 - y0)
}
