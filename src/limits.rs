use crate::{Error, Resource};

const MAX_SIDE: u32 = 4096;
const MAX_PIXELS: u64 = 16_777_216;
const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_WORK: u64 = 1_000_000_000;

/// Per-image ceilings, also applied to copy scratch buffers.
///
/// These do not account aggregate live project memory. A job-level payload
/// accountant is required before accepting untrusted jobs or shared cels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    side: u32,
    pixels: u64,
    bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            side: MAX_SIDE,
            pixels: MAX_PIXELS,
            bytes: MAX_BYTES,
        }
    }
}

impl Limits {
    /// All ceilings must be positive and no greater than the design defaults.
    pub fn new(side: u32, pixels: u64, bytes: u64) -> Result<Self, Error> {
        for (invalid, resource) in [
            (side == 0 || side > MAX_SIDE, Resource::Side),
            (pixels == 0 || pixels > MAX_PIXELS, Resource::Pixels),
            (bytes == 0 || bytes > MAX_BYTES, Resource::Bytes),
        ] {
            if invalid {
                return Err(Error::Limit(resource));
            }
        }
        Ok(Self {
            side,
            pixels,
            bytes,
        })
    }

    pub(crate) fn image_len(self, width: u32, height: u32) -> Result<usize, Error> {
        if width == 0 || height == 0 {
            return Err(Error::Dimensions);
        }
        if width > self.side || height > self.side {
            return Err(Error::Limit(Resource::Side));
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > self.pixels {
            return Err(Error::Limit(Resource::Pixels));
        }
        self.check_bytes(pixels)?;
        usize::try_from(pixels).map_err(|_| Error::Limit(Resource::Pixels))
    }

    pub(crate) fn check_bytes(self, pixels: u64) -> Result<(), Error> {
        let bytes = pixels.checked_mul(4).ok_or(Error::Limit(Resource::Bytes))?;
        if bytes > self.bytes {
            return Err(Error::Limit(Resource::Bytes));
        }
        Ok(())
    }
}

/// Monotonic pixel-visit budget, shared across calls by the caller.
/// Failed reservations leave it unchanged; later allocation failures may consume
/// already charged work. It is not a process time or memory limit.
#[derive(Debug)]
pub struct Work {
    remaining: u64,
}

impl Default for Work {
    fn default() -> Self {
        Self {
            remaining: MAX_WORK,
        }
    }
}

impl Work {
    pub fn new(units: u64) -> Result<Self, Error> {
        if units > MAX_WORK {
            return Err(Error::Limit(Resource::Work));
        }
        Ok(Self { remaining: units })
    }

    pub fn remaining(&self) -> u64 {
        self.remaining
    }

    pub(crate) fn charge(&mut self, units: u64) -> Result<(), Error> {
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or(Error::Limit(Resource::Work))?;
        Ok(())
    }
}
