#![forbid(unsafe_code)]
//! Deterministic raster primitives, bounded JSON and static PNG.
//! The typed agent interface is not implemented yet.

mod checksum;
mod color_edit;
pub mod compare;
mod deflate;
mod error;
pub mod inspect;
pub mod json;
mod limits;
mod model;
pub mod palette;
pub mod png;
mod raster;
pub mod render;
mod scale;

pub use color_edit::PaletteMismatch;
pub use error::{Error, Resource};
pub use limits::{Limits, Work};
pub use model::{Image, Pixel, Rect};
pub use raster::{Blend, blend};
