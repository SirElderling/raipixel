use std::fmt;

/// Resource whose configured ceiling would be exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Side,
    Pixels,
    Bytes,
    Work,
}

/// Library failures. A future interface will add operation/target context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Dimensions,
    Rectangle,
    PixelCount,
    PaletteEmpty,
    PaletteTooMany { limit: u16, distinct: u32 },
    Bounds,
    Limit(Resource),
    Allocation,
}

impl Error {
    pub fn code(self) -> &'static str {
        match self {
            Self::Dimensions | Self::Rectangle | Self::PixelCount => "E_ARGUMENT",
            Self::PaletteEmpty => "E_PALETTE_EMPTY",
            Self::PaletteTooMany { .. } => "E_PALETTE_TOO_MANY",
            Self::Bounds => "E_BOUNDS",
            Self::Limit(_) => "E_LIMIT",
            Self::Allocation => "E_ALLOC",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dimensions => f.write_str("Image dimensions must be positive"),
            Self::Rectangle => f.write_str("Rectangle dimensions must be positive"),
            Self::PixelCount => f.write_str("Pixel count does not match image dimensions"),
            Self::PaletteEmpty => f.write_str("No visible colors are available for extraction"),
            Self::PaletteTooMany { limit, distinct } => write!(
                f,
                "Found {distinct} visible RGB colors, exceeding maximum {limit}",
            ),
            Self::Bounds => f.write_str("Rectangle exceeds image bounds"),
            Self::Limit(resource) => write!(f, "{resource:?} limit exceeded"),
            Self::Allocation => f.write_str("Unable to reserve image memory"),
        }
    }
}

impl std::error::Error for Error {}
