#![forbid(unsafe_code)]
//! Deterministic raster primitives, bounded JSON and static PNG.
//! The typed agent interface is not implemented yet.

mod checksum;
mod deflate;
mod error;
pub mod json;
mod limits;
mod model;
pub mod png;
mod raster;
pub mod render;

pub use error::{Error, Resource};
pub use limits::{Limits, Work};
pub use model::{Image, Pixel, Rect};
pub use raster::{Blend, blend};
