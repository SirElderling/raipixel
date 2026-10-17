//! Opt-in balanced exact-color indexed PNG encoder.
//!
//! Three independently encoded filters (None, Sub, per-row adaptive-64),
//! stable minimum-byte winner. Existing accepted PNG API remains unchanged.
//! No third-party production dependencies. C24 experiment register defines
//! the fixed corpus and provenance; size/regression results are corpus-only.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    None,
    Sub,
    Auto64,
}

fn encode_mode(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
    mode: Filter,
) -> Result<bool, Error> {
    let pixels = image.pixels();
    work.charge(pixels.len() as u64)?;
    let mut colors = Vec::<[u8; 4]>::new();
    // No implicit growth in the at-most-256-entry exact-RGBA palette.
    colors.try_reserve_exact(256).map_err(|_| crate::Error::Allocation)?;
    let mut slots = [u16::MAX; 4096];
    let slot_for = |sample: [u8; 4]| -> usize {
        (u32::from_le_bytes(sample).wrapping_mul(0x9e37_79b1) >> 20) as usize
    };
    for &sample in pixels {
        let mut slot = slot_for(sample);
        loop {
            let value = slots[slot];
            if value == u16::MAX {
                if colors.len() == 256 {
                    return Ok(false);
                }
                slots[slot] = colors.len() as u16;
                colors.push(sample);
                break;
            }
            if colors[usize::from(value)] == sample {
                break;
            }
            slot = (slot + 1) & 4095;
        }
    }

    let width = image.width as usize;
    let depth: usize = match colors.len() {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    };
    let row_bytes = (width * depth).div_ceil(8);
    let stride = row_bytes + 1;
    let raw_size = stride * image.height as usize;
    let blocks = raw_size.div_ceil(crate::deflate::BLOCK);
    work.charge((raw_size * 16 + blocks * (8192 + 64)) as u64)?;
    let capacity = raw_size.min(crate::deflate::BLOCK);
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity)
        .map_err(|_| crate::Error::Allocation)?;
    let mut encoder = crate::deflate::quality::Encoder::new(capacity)?;
    let mut palette = Vec::new();
    palette
        .try_reserve_exact(colors.len() * 3)
        .map_err(|_| crate::Error::Allocation)?;
    let mut alpha = Vec::new();
    alpha
        .try_reserve_exact(colors.len())
        .map_err(|_| crate::Error::Allocation)?;
    for color in &colors {
        palette.extend_from_slice(&color[..3]);
        alpha.push(color[3]);
    }

    let mut ihdr = [0u8; 13];
    ihdr[..4].copy_from_slice(&image.width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&image.height.to_be_bytes());
    ihdr[8] = depth as u8;
    ihdr[9] = 3;
    output.write_all(SIGNATURE)?;
    write_chunk(output, b"IHDR", &ihdr)?;
    write_chunk(output, b"PLTE", &palette)?;
    if alpha.iter().any(|&value| value != 255) {
        write_chunk(output, b"tRNS", &alpha)?;
    }
    write_chunk(output, b"IDAT", &[0x78, 0x01])?;

    let mut offset = 0;
    let mut adler = Adler::new();
    let mut previous = Vec::new();
    let mut current = Vec::new();
    if mode != Filter::None {
        previous.try_reserve_exact(row_bytes).map_err(|_| crate::Error::Allocation)?;
        current.try_reserve_exact(row_bytes).map_err(|_| crate::Error::Allocation)?;
        previous.resize(row_bytes, 0);
        current.resize(row_bytes, 0);
    }
    let mut left_pixel = 0u8;
    let mut selected_filter = 0u8;
    while offset < raw_size {
        let len = (raw_size - offset).min(capacity);
        raw.resize(len, 0);
        for (i, byte) in raw.iter_mut().enumerate() {
            let position = offset + i;
            let row = position / stride;
            let col = position % stride;
            // Both filter markers and packed bytes must be reset across
            // DEFLATE blocks, since resize can reuse the same scratch bytes.
            *byte = 0;
            if col != 0 {
                let per_byte = 8 / depth;
                let first_pixel = (col - 1) * per_byte;
                for j in 0..per_byte {
                    if first_pixel + j >= width {
                        break;
                    }
                    let sample = pixels[row * width + first_pixel + j];
                    let mut slot = slot_for(sample);
                    loop {
                        let value = slots[slot];
                        if value == u16::MAX {
                            return Err(Error::Chunk);
                        }
                        if colors[usize::from(value)] == sample {
                            *byte |= (value as u8) << (8 - depth * (j + 1));
                            break;
                        }
                        slot = (slot + 1) & 4095;
                    }
                }
            }
        }
        if mode != Filter::None {
            for i in 0..raw.len() {
                let position = offset + i;
                let column = position % stride;
                if column == 0 {
                    if position != 0 {
                        std::mem::swap(&mut previous, &mut current);
                    }
                    left_pixel = 0;
                    selected_filter = match mode {
                        Filter::None => 0,
                        Filter::Sub => 1,
                        Filter::Auto64 => {
                            let sample_count = row_bytes.min(raw.len() - i - 1).min(64);
                            work.charge((sample_count as u64) * 4)?;
                            let mut best_filter = 0;
                            let mut best_score = u64::MAX;
                            for filter in [0u8, 1, 2, 4] {
                                let mut score = 0u64;
                                for j in 0..sample_count {
                                    let original = raw[i + j + 1];
                                    let left = if j != 0 { raw[i + j] } else { 0 };
                                    let above = previous[j];
                                    let upper_left = if j != 0 { previous[j - 1] } else { 0 };
                                    let predicted = match filter {
                                        0 => 0,
                                        1 => left,
                                        2 => above,
                                        _ => paeth(left, above, upper_left),
                                    };
                                    let residual = original.wrapping_sub(predicted);
                                    score += if residual > 127 {
                                        256 - u64::from(residual)
                                    } else {
                                        u64::from(residual)
                                    };
                                }
                                if score < best_score {
                                    best_score = score;
                                    best_filter = filter;
                                }
                            }
                            best_filter
                        }
                    };
                    raw[i] = selected_filter;
                } else {
                    let original = raw[i];
                    let above = previous[column - 1];
                    let upper_left = if column > 1 { previous[column - 2] } else { 0 };
                    let predicted = match selected_filter {
                        0 => 0,
                        1 => left_pixel,
                        2 => above,
                        4 => paeth(left_pixel, above, upper_left),
                        _ => unreachable!(),
                    };
                    raw[i] = original.wrapping_sub(predicted);
                    current[column - 1] = original;
                    left_pixel = original;
                }
            }
        }
        adler.update(&raw);
        let compressed = encoder.block_history(&raw, offset + len == raw_size, work)?;
        write_chunk(output, b"IDAT", compressed)?;
        offset += len;
    }
    write_chunk(output, b"IDAT", &adler.value().to_be_bytes())?;
    write_chunk(output, b"IEND", &[])?;
    Ok(true)
}

/// Encode an exact-color indexed PNG without publishing partial bytes on
/// encoding, allocation, or work-budget failure.
///
/// This transactional output wrapper preserves the existing indexed encoder's
/// output bytes and compression strategy. A false return means >256 exact RGBA
/// samples; no bytes are written. An error from the caller's output writer
/// during the final commit can still leave partial output.
///
/// Its logical staging cap is bounded by the worst-case indexed PNG size.
/// This is not a strict process-wide heap or RSS guarantee.
pub fn encode_indexed_staged(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
) -> Result<bool, Error> {
    let width = image.width as usize;
    let height = image.height as usize;
    let max_raw = width.checked_add(1)
        .and_then(|stride| stride.checked_mul(height))
        .ok_or(crate::Error::Limit(crate::Resource::Bytes))?;
    let blocks = max_raw.div_ceil(crate::deflate::BLOCK);
    // 8-bit indexed input is the largest packed palette representation.
    // Stored DEFLATE needs 5 extra bytes per block, IDAT adds 12;
    // 2048 covers header, palette, transparency, zlib, Adler and IEND.
    let max_output = blocks.checked_mul(17)
        .and_then(|v| v.checked_add(max_raw))
        .and_then(|v| v.checked_add(2048))
        .ok_or(crate::Error::Limit(crate::Resource::Bytes))?;
    image.limits.check_buffer(max_output as u64)?;

    // The flags preserve resource-error identity through the Write trait.
    struct Stage {
        bytes: Vec<u8>,
        max: usize,
        exhausted: bool,
        allocation_failed: bool,
    }
    impl std::io::Write for Stage {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len().checked_add(data.len())
                .is_none_or(|n| n > self.max)
            {
                self.exhausted = true;
                return Err(std::io::Error::other("indexed PNG staging limit"));
            }
            if self.bytes.try_reserve(data.len()).is_err() {
                self.allocation_failed = true;
                return Err(std::io::Error::other("indexed PNG staging allocation"));
            }
            self.bytes.extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut stage = Stage {
        bytes: Vec::new(),
        max: max_output,
        exhausted: false,
        allocation_failed: false,
    };
    match encode_indexed(image, &mut stage, work) {
        Ok(false) => Ok(false),
        Ok(true) => {
            output.write_all(&stage.bytes)?;
            Ok(true)
        }
        Err(_) if stage.allocation_failed => {
            Err(crate::Error::Allocation.into())
        }
        Err(_) if stage.exhausted => {
            Err(crate::Error::Limit(crate::Resource::Bytes).into())
        }
        Err(error) => Err(error),
    }
}


struct Stage {
    bytes: Vec<u8>,
    max: usize,
    allocation_failed: bool,
    exhausted: bool,
    // Private deterministic fault injection, never exposed via public API.
    fail_next_reserve: bool,
}
impl Stage {
    fn new(max: usize, fail_next_reserve: bool) -> Self {
        Self { bytes: Vec::new(), max, allocation_failed: false, exhausted: false, fail_next_reserve }
    }
}
impl std::io::Write for Stage {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().checked_add(data.len()).is_none_or(|n| n > self.max) {
            self.exhausted = true;
            return Err(std::io::Error::other("balanced PNG staging limit"));
        }
        if self.bytes.len() + data.len() > self.bytes.capacity() {
            // Explicitly bound requested capacity. A fallible exact reserve
            // avoids unconstrained geometric overgrowth in the staged PNG.
            let needed = self.bytes.len() + data.len();
            let desired = self.bytes.capacity()
                .saturating_mul(2)
                .max(4096)
                .max(needed)
                .min(self.max);
            // Fault injection targets an actual growth request, not writes
            // already covered by an existing allocation.
            if self.fail_next_reserve {
                self.fail_next_reserve = false;
                self.allocation_failed = true;
                return Err(std::io::Error::other("injected PNG staging allocation failure"));
            }
            if self.bytes.try_reserve_exact(desired - self.bytes.len()).is_err() {
                self.allocation_failed = true;
                return Err(std::io::Error::other("balanced PNG staging allocation"));
            }
            // Allocators may report more capacity than requested even for
            // try_reserve_exact. Treat that as an over-limit resource failure.
            if self.bytes.capacity() > self.max {
                self.exhausted = true;
                return Err(std::io::Error::other("balanced PNG staging capacity limit"));
            }
        }
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

// Conservative capacity-planning gate, not an allocator-independent heap cap.
// Allow room for both buffered PNG outputs, the input RGBA image, and the
// fallibly reserved matcher history/links/tokens/Huffman scratch. A Vec's
// actual capacity may exceed its requested reservation; true RSS limits
// require allocation-level tracking beyond this deliberately simple gate.
fn checked_balanced_memory_plan(image: &Image, raw: usize, ceiling: usize) -> Result<(), Error> {
    let limit = || Error::Resource(crate::Error::Limit(crate::Resource::Bytes));
    let block = raw.min(crate::deflate::BLOCK);
    let joined = block.checked_add(32_768).ok_or_else(limit)?;
    let rgba_pixels = (image.width as usize)
        .checked_mul(image.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(limit)?;
    // Vec::try_reserve may grow geometrically. Allow up to 2x each of the
    // best and current PNG staging buffers rather than just their lengths.
    let output_capacity = ceiling.checked_mul(4).ok_or_else(limit)?;
    let links = joined.checked_mul(std::mem::size_of::<u32>()).ok_or_else(limit)?;
    let tokens = block.checked_mul(std::mem::size_of::<u16>()).ok_or_else(limit)?;
    let encoder_output = block.checked_mul(9).and_then(|n| n.checked_div(8))
        .and_then(|n| n.checked_add(16))
        .ok_or_else(limit)?;
    let rows = (image.width as usize).checked_mul(2).ok_or_else(limit)?;
    let parts = [
        rgba_pixels, output_capacity, block, joined, links, tokens,
        encoder_output, 32_768, 8192 * 8, rows,
        8192, // Palette, color table and stack hash slots.
        256 * 1024, // Conservative per-block Huffman and allocator headroom.
    ];
    let planned = parts.into_iter().try_fold(0usize, |total, part| {
        total.checked_add(part).ok_or_else(limit)
    })?;
    image.limits.check_buffer(planned as u64).map_err(Error::Resource)
}

/// Experimental opt-in balanced PNG compression.
///
/// Uses three measured depth-64 lazy/dynamic Huffman modes, chooses the
/// smallest complete lossless indexed PNG, and publishes only on success.
/// This does NOT promise the universally smallest possible PNG; the retained
/// six-mode reference can beat this preset on some inputs. The old writer's
/// compression bytes and public API remain unchanged.
///
/// False means more than 256 distinct exact RGBA values; no caller bytes
/// are written. Work and stage allocation failures do not publish bytes.
/// A real failure from the caller's Write may still leave partial output.
/// The largest two staging PNGs have checked logical size limits, but an
/// OS-wide peak heap bound/fault-injected OOM certification remains pending.
pub fn encode_indexed_balanced(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
) -> Result<bool, Error> {
    encode_balanced_inner(image, output, work, None)
}

// Private only: fault injection never leaks into the public encoding surface.
fn encode_balanced_inner(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
    injected_allocation_pass: Option<usize>,
) -> Result<bool, Error> {
    let width = image.width as usize;
    let height = image.height as usize;
    let raw = width.checked_add(1).and_then(|x|x.checked_mul(height))
        .ok_or(crate::Error::Limit(crate::Resource::Bytes))?;
    let blocks = raw.div_ceil(crate::deflate::BLOCK);
    let ceiling = blocks.checked_mul(17).and_then(|x|x.checked_add(raw))
        .and_then(|x|x.checked_add(2048))
        .ok_or(crate::Error::Limit(crate::Resource::Bytes))?;
    let staging_max = ceiling.checked_mul(2)
        .ok_or(crate::Error::Limit(crate::Resource::Bytes))?;
    image.limits.check_buffer(staging_max as u64)?;
    // Refuse infeasible aggregate scratch plans before charging Work or
    // touching output. Conservative planning is not RSS certification.
    checked_balanced_memory_plan(image, raw, ceiling)?;

    let mut best: Option<Vec<u8>> = None;
    for (pass, mode) in [Filter::None, Filter::Sub, Filter::Auto64].into_iter().enumerate() {
        let mut current = Stage::new(ceiling, injected_allocation_pass == Some(pass));
        let supported = match encode_mode(image, &mut current, work, mode) {
            Ok(value) => value,
            Err(_) if current.allocation_failed => {
                return Err(crate::Error::Allocation.into());
            }
            Err(_) if current.exhausted => {
                return Err(crate::Error::Limit(crate::Resource::Bytes).into());
            }
            Err(error) => return Err(error),
        };
        if !supported { return Ok(false); }
        if best.as_ref().is_none_or(|value| current.bytes.len() < value.len()) {
            best = Some(current.bytes);
        }
    }
    if let Some(encoded) = best {
        output.write_all(&encoded)?;
        Ok(true)
    } else {
        Err(Error::Chunk)
    }
}

#[cfg(test)]
mod fault_injection_tests {
    use super::*;

    #[test]
    fn staged_output_growth_stays_within_reported_byte_capacity() {
        use std::io::Write;

        let mut stage = Stage::new(8192, false);
        for _ in 0..32 {
            stage.write_all(&[17; 256]).unwrap();
            assert!(stage.bytes.capacity() <= stage.max);
        }
        assert_eq!(stage.bytes.len(), 8192);
        let former = stage.bytes.clone();
        assert!(stage.write_all(&[1]).is_err());
        assert_eq!(stage.bytes, former, "size-limit rejection altered staged bytes");
        assert!(stage.exhausted);
        assert!(!stage.allocation_failed);
    }

    #[test]
    fn scratch_capacity_preflight_rejects_before_work_or_publication() {
        let samples = vec![[23, 57, 91, 0]; 16 * 16];
        let constrained = crate::Limits::new(64, 4096, 256 * 1024).unwrap();
        let image = Image::from_pixels(
            16, 16, &samples, constrained, &mut Work::default()
        ).unwrap();
        let mut writer = vec![0xfa, 0xce];
        let mut work = Work::default();
        let before = work.remaining();
        let error = encode_indexed_balanced(&image, &mut writer, &mut work).unwrap_err();
        assert_eq!(error.code(), "E_LIMIT");
        assert_eq!(writer, [0xfa, 0xce]);
        assert_eq!(work.remaining(), before, "preflight consumed Work units");

        let roomy = crate::Limits::new(64, 4096, 1024 * 1024).unwrap();
        let image = Image::from_pixels(
            16, 16, &samples, roomy, &mut Work::default()
        ).unwrap();
        let mut png = Vec::new();
        assert!(encode_indexed_balanced(
            &image, &mut png, &mut Work::default()
        ).unwrap());
        assert_eq!(
            crate::png::decode(
                &png, crate::Limits::default(), &mut Work::default()
            ).unwrap().image,
            image
        );
    }

    #[test]
    fn every_staging_allocation_failure_is_a_resource_error_without_publication() {
        let pixels: Vec<[u8; 4]> = (0..(32 * 32))
            .map(|i| {
                let color = (i % 7) as u8;
                [color, color.wrapping_mul(17), color.wrapping_mul(31), 0]
            })
            .collect();
        let image = Image::from_pixels(
            32,
            32,
            &pixels,
            crate::Limits::default(),
            &mut Work::default(),
        ).unwrap();
        for pass in 0..3 {
            let mut caller = vec![0xa5, 0x5d];
            let error = encode_balanced_inner(
                &image,
                &mut caller,
                &mut Work::default(),
                Some(pass),
            ).unwrap_err();
            assert_eq!(error.code(), "E_ALLOC", "allocation fault at pass {pass}");
            assert_eq!(caller, [0xa5, 0x5d], "pass {pass} published partial output");
        }
        let mut normal = Vec::new();
        assert!(encode_balanced_inner(
            &image, &mut normal, &mut Work::default(), None
        ).unwrap());
        assert_eq!(
            crate::png::decode(
                &normal,
                crate::Limits::default(),
                &mut Work::default()
            ).unwrap().image,
            image
        );
    }
}
