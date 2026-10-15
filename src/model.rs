use crate::{Error, Limits, Work};

/// Straight RGBA8, including meaningful RGB samples when alpha is zero.
pub type Pixel = [u8; 4];

/// Positive, half-open rectangle. Endpoints are computed in i64.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Result<Self, Error> {
        if width == 0 || height == 0 {
            return Err(Error::Rectangle);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub(crate) fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub(crate) fn bounds(self) -> [i64; 4] {
        let (x, y) = (i64::from(self.x), i64::from(self.y));
        [x, y, x + i64::from(self.width), y + i64::from(self.height)]
    }
}

/// Owned contiguous row-major pixels. No infallible Clone: copies are budgeted.
#[derive(Debug)]
pub struct Image {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<Pixel>,
    pub(crate) limits: Limits,
}

// Limits affect future allocation policy, not semantic image equality.
impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        self.size() == other.size() && self.pixels == other.pixels
    }
}
impl Eq for Image {}

impl Image {
    pub fn new(width: u32, height: u32, limits: Limits, work: &mut Work) -> Result<Self, Error> {
        let len = limits.image_len(width, height)?;
        work.charge(len as u64)?;
        let mut pixels = reserve(len)?;
        pixels.resize(len, [0; 4]);
        Ok(Self {
            width,
            height,
            pixels,
            limits,
        })
    }

    pub fn from_pixels(
        width: u32,
        height: u32,
        samples: &[Pixel],
        limits: Limits,
        work: &mut Work,
    ) -> Result<Self, Error> {
        let len = limits.image_len(width, height)?;
        if samples.len() != len {
            return Err(Error::PixelCount);
        }
        work.charge(len as u64)?;
        let mut pixels = reserve(len)?;
        pixels.extend_from_slice(samples);
        Ok(Self {
            width,
            height,
            pixels,
            limits,
        })
    }

    pub fn size(&self) -> [u32; 2] {
        [self.width, self.height]
    }
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<Pixel> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.pixels[self.index(x, y)])
    }

    pub fn try_copy(&self, work: &mut Work) -> Result<Self, Error> {
        Self::from_pixels(self.width, self.height, &self.pixels, self.limits, work)
    }

    pub(crate) fn index(&self, x: u32, y: u32) -> usize {
        y as usize * self.width as usize + x as usize
    }

    pub(crate) fn coverage(&self, rect: Rect, clip: bool) -> Result<[u32; 4], Error> {
        let [x0, y0, x1, y1] = rect.bounds();
        let (w, h) = (i64::from(self.width), i64::from(self.height));
        if !clip && (x0 < 0 || y0 < 0 || x1 > w || y1 > h) {
            return Err(Error::Bounds);
        }
        Ok([
            x0.clamp(0, w) as u32,
            y0.clamp(0, h) as u32,
            x1.clamp(0, w) as u32,
            y1.clamp(0, h) as u32,
        ])
    }
}

pub(crate) fn reserve(len: usize) -> Result<Vec<Pixel>, Error> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(len)
        .map_err(|_| Error::Allocation)?;
    Ok(pixels)
}
