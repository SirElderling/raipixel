//! Exact, single-frame layer compositing over a bounded RGBA canvas.
//!
//! Layers are supplied bottom-to-top. This is a read-only view, not an
//! additional ownership model or a project persistence abstraction.

use crate::{Blend, Error, Image, Limits, Rect, Work};

/// A stored cel and its layer visibility, opacity and canvas placement.
pub struct Layer<'a> {
    pub image: &'a Image,
    pub at: [i32; 2],
    pub visible: bool,
    pub opacity: u8,
}

/// Render one frame. The first visible cel uses Copy, retaining RGB even where
/// alpha becomes zero. Later visible cels use the defined integer source-over
/// operation. Invisible cels are not read or charged as pixel work.
///
/// Every input image remains unchanged, including its hidden RGB. Output
/// allocation and all declared source rectangles consume the caller's shared
/// work budget. The caller receives no partially rendered image on failure.
pub fn frame(
    size: [u32; 2],
    layers: &[Layer<'_>],
    limits: Limits,
    work: &mut Work,
) -> Result<Image, Error> {
    let mut result = Image::new(size[0], size[1], limits, work)?;
    let mut bottom = true;
    for layer in layers {
        if !layer.visible {
            continue;
        }
        let [width, height] = layer.image.size();
        let source = Rect::new(0, 0, width, height)?;
        result.stamp(
            layer.image,
            source,
            layer.at,
            if bottom { Blend::Copy } else { Blend::Over },
            layer.opacity,
            true,
            work,
        )?;
        bottom = false;
    }
    Ok(result)
}
