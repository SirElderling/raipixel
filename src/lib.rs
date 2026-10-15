#![forbid(unsafe_code)]
//! Deterministic raster primitives and bounded JSON. The agent interface is not implemented yet.

mod checksum;
mod deflate;
mod error;
pub mod json;
mod limits;
mod model;
pub mod png;
mod raster;

pub use error::{Error, Resource};
pub use limits::{Limits, Work};
pub use model::{Image, Pixel, Rect};
pub use raster::{Blend, blend};
