//! Exact and deterministic color edits. No heap lookup table or color-space conversion.
use crate::{Error, Image, Pixel, Rect, Work};

impl Image {
    /// Apply up to 256 simultaneous exact mappings to original pixels.
    ///
    /// When preserving alpha, only RGB is compared and changed; input alpha
    /// is ignored. Otherwise all four samples participate in matching and
    /// replacement. Lookup sorting is local and never changes pair priority.
    /// Duplicate source colors and invalid regions fail before any mutation.
    /// Returns the number of actually changed pixels, not matched pixels.
    pub fn map_colors(
        &mut self,
        pairs: &[(Pixel, Pixel)],
        preserve_alpha: bool,
        region: Option<Rect>,
        work: &mut Work,
    ) -> Result<u64, Error> {
        if pairs.is_empty() || pairs.len() > 256 {
            return Err(Error::PixelCount);
        }

        // Fixed upper bound keeps both sorted lookup and scratch on the stack.
        let mut sorted = [([0u8; 4], [0u8; 4]); 256];
        for (entry, &(mut from, mut to)) in sorted.iter_mut().zip(pairs) {
            if preserve_alpha {
                from[3] = 0;
                to[3] = 0;
            }
            *entry = (from, to);
        }
        let table = &mut sorted[..pairs.len()];
        table.sort_unstable_by_key(|(from, _)| *from);
        if table.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::PixelCount);
        }

        self.edit_region(region, work, |sample| {
            let mut key = sample;
            if preserve_alpha {
                key[3] = 0;
            }
            match table.binary_search_by_key(&key, |(from, _)| *from) {
                Ok(index) => {
                    let mut output = table[index].1;
                    if preserve_alpha {
                        output[3] = sample[3];
                    }
                    output
                }
                Err(_) => sample,
            }
        })
    }

    /// Set alpha to zero for exact RGB or RGBA matches, retaining all RGB bytes.
    pub fn alpha_key(
        &mut self,
        color: Pixel,
        match_alpha: bool,
        region: Option<Rect>,
        work: &mut Work,
    ) -> Result<u64, Error> {
        self.edit_region(region, work, |mut sample| {
            if sample[..3] == color[..3] && (!match_alpha || sample[3] == color[3]) {
                sample[3] = 0;
            }
            sample
        })
    }

    /// Convert alpha to binary at an explicit threshold, preserving hidden RGB.
    /// Samples below the threshold become zero; all others become 255.
    pub fn alpha_threshold(
        &mut self,
        threshold: u8,
        region: Option<Rect>,
        work: &mut Work,
    ) -> Result<u64, Error> {
        self.edit_region(region, work, |mut sample| {
            sample[3] = if sample[3] < threshold { 0 } else { 255 };
            sample
        })
    }

    fn edit_region(
        &mut self,
        region: Option<Rect>,
        work: &mut Work,
        mut transform: impl FnMut(Pixel) -> Pixel,
    ) -> Result<u64, Error> {
        let [x0, y0, x1, y1] = match region {
            Some(rect) => self.coverage(rect, false)?,
            None => [0, 0, self.width, self.height],
        };
        work.charge(u64::from(x1 - x0) * u64::from(y1 - y0))?;
        let mut changed = 0u64;
        for y in y0..y1 {
            let start = self.index(x0, y);
            let end = start + (x1 - x0) as usize;
            for sample in &mut self.pixels_mut()[start..end] {
                let next = transform(*sample);
                if next != *sample {
                    *sample = next;
                    changed += 1;
                }
            }
        }
        Ok(changed)
    }
}
