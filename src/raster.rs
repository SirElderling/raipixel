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
            self.pixels[start..end].fill(color);
        }
        Ok(rect.area() - u64::from(x1 - x0) * u64::from(y1 - y0))
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
            &source.pixels,
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
            snapshot.extend_from_slice(&self.pixels[start..start + rect.width as usize]);
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
                self.pixels[index] = blend(src, self.pixels[index], opacity, mode);
            }
        }
    }
}

fn discarded(rect: Rect, coverage: [u32; 4]) -> u64 {
    let [x0, y0, x1, y1] = coverage;
    rect.area() - u64::from(x1 - x0) * u64::from(y1 - y0)
}
