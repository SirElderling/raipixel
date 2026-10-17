//! Exact deterministic color edits with bounded, allocation-free lookups.
use crate::{Error, Image, Pixel, Rect, Work};

impl Image {
    /// Apply up to 256 simultaneous exact mappings to original pixels.
    ///
    /// When preserving alpha, only RGB is compared and changed; input alpha
    /// is ignored. Otherwise all four samples participate in matching and
    /// replacement. Lookup order is deterministic and never changes pair priority.
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

        // Bounded open addressing is measurably faster on the native sprite
        // corpus than repeated comparisons or binary search. Keep the sorted
        // table as a guaranteed worst-case fallback for adversarial collisions.
        const SLOTS: usize = 2048;
        const PROBES: usize = 16;
        let mut lookup = [None::<(Pixel, Pixel)>; SLOTS];
        let mut fast_path = true;
        for &(from, to) in table.iter() {
            let mut slot = (u32::from_be_bytes(from).wrapping_mul(0x9E37_79B1) as usize)
                & (SLOTS - 1);
            let mut inserted = false;
            for _ in 0..PROBES {
                if lookup[slot].is_none() {
                    lookup[slot] = Some((from, to));
                    inserted = true;
                    break;
                }
                slot = (slot + 1) & (SLOTS - 1);
            }
            if !inserted {
                fast_path = false;
                break;
            }
        }

        self.edit_region(region, work, |sample| {
            let mut key = sample;
            if preserve_alpha {
                key[3] = 0;
            }
            if fast_path {
                let mut slot = (u32::from_be_bytes(key).wrapping_mul(0x9E37_79B1) as usize)
                    & (SLOTS - 1);
                for _ in 0..PROBES {
                    match lookup[slot] {
                        Some((from, mut to)) if from == key => {
                            if preserve_alpha {
                                to[3] = sample[3];
                            }
                            return to;
                        }
                        None => return sample,
                        _ => slot = (slot + 1) & (SLOTS - 1),
                    }
                }
            }
            // Any missing hash key or overlong collision chain uses the exact
            // fallback; no input can force an unbounded pixel-level search.
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


    /// Find the first visible RGB sample that is not in the supplied palette.
    ///
    /// Fully transparent pixels retain their hidden RGB and do not violate a
    /// visible-palette restriction. One read per pixel is charged before the
    /// scan; lookup uses a fixed 256-entry stack table and binary search.
    pub fn first_visible_color_outside_palette(
        &self,
        allowed_rgb: &[[u8; 3]],
        work: &mut Work,
    ) -> Result<Option<([u32; 2], [u8; 3])>, Error> {
        if allowed_rgb.is_empty() || allowed_rgb.len() > 256 {
            return Err(Error::PixelCount);
        }
        let mut table = [[0u8; 3]; 256];
        table[..allowed_rgb.len()].copy_from_slice(allowed_rgb);
        let table = &mut table[..allowed_rgb.len()];
        table.sort_unstable();
        work.charge(self.pixels().len() as u64)?;
        for (index, pixel) in self.pixels().iter().enumerate() {
            if pixel[3] == 0 {
                continue;
            }
            let rgb = [pixel[0], pixel[1], pixel[2]];
            if table.binary_search(&rgb).is_err() {
                let x = (index % self.width as usize) as u32;
                let y = (index / self.width as usize) as u32;
                return Ok(Some(([x, y], rgb)));
            }
        }
        Ok(None)
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
