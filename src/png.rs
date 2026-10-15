//! Static PNG import profile and streaming RGBA8 PNG output.
//! Chunk framing/CRC, zlib compression, and row reconstruction remain separate.
use crate::checksum::{Adler, crc32};
use crate::{Image, Limits, Work};
use std::borrow::Cow;
#[derive(Debug)]
pub enum Error {
    Signature,
    Chunk,
    Crc,
    Filter,
    Deflate,
    Adler,
    UnsupportedDepth,
    UnsupportedAnimation,
    Unsupported,
    Resource(crate::Error),
    Io(std::io::Error),
}
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Signature => "E_PNG_SIGNATURE",
            Self::Chunk => "E_PNG_CHUNK",
            Self::Crc => "E_PNG_CRC",
            Self::Filter => "E_PNG_FILTER",
            Self::Deflate => "E_PNG_DEFLATE",
            Self::Adler => "E_PNG_ADLER",
            Self::UnsupportedDepth => "E_UNSUPPORTED_DEPTH",
            Self::UnsupportedAnimation => "E_UNSUPPORTED_ANIMATION",
            Self::Unsupported => "E_UNSUPPORTED_PNG",
            Self::Resource(err) => err.code(),
            Self::Io(_) => "E_IO",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Signature => "Invalid PNG signature",
            Self::Chunk => "Invalid PNG chunk, ordering, dimensions or samples",
            Self::Crc => "PNG chunk CRC mismatch",
            Self::Filter => "Invalid PNG scanline filter",
            Self::Deflate => "Invalid or out-of-bounds zlib/DEFLATE stream",
            Self::Adler => "Zlib Adler32 mismatch",
            Self::UnsupportedDepth => "16-bit PNG import is not supported by this profile",
            Self::UnsupportedAnimation => "Animated PNG import is not supported",
            Self::Unsupported => "Unknown critical PNG chunk",
            Self::Resource(err) => return std::fmt::Display::fmt(err, f),
            Self::Io(err) => return std::fmt::Display::fmt(err, f),
        };
        write!(f, "{}: {message}", self.code())
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}
impl From<crate::Error> for Error {
    fn from(value: crate::Error) -> Self {
        Self::Resource(value)
    }
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<crate::deflate::Error> for Error {
    fn from(value: crate::deflate::Error) -> Self {
        match value {
            crate::deflate::Error::Data => Self::Deflate,
            crate::deflate::Error::Adler => Self::Adler,
            crate::deflate::Error::Resource(err) => Self::Resource(err),
        }
    }
}
#[derive(Debug)]
pub struct Decoded {
    pub image: Image,
    pub uninterpreted_color: bool,
    pub skipped_ancillary: bool,
    pub trailing_idat_bytes: usize,
}

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const MAX_INPUT: usize = 128 * 1024 * 1024;
const ADAM7: [[usize; 4]; 7] = [
    [0, 0, 8, 8],
    [4, 0, 8, 8],
    [0, 4, 4, 8],
    [2, 0, 4, 4],
    [0, 2, 2, 4],
    [1, 0, 2, 2],
    [0, 1, 1, 2],
];
const PLAIN: [[usize; 4]; 1] = [[0, 0, 1, 1]];

struct Header {
    width: u32,
    height: u32,
    depth: usize,
    color: u8,
    channels: usize,
    interlace: bool,
}
impl Header {
    fn parse(data: &[u8], limits: Limits) -> Result<Self, Error> {
        if data.len() != 13 {
            return Err(Error::Chunk);
        }
        let width = u32::from_be_bytes(data[..4].try_into().map_err(|_| Error::Chunk)?);
        let height = u32::from_be_bytes(data[4..8].try_into().map_err(|_| Error::Chunk)?);
        if width == 0
            || height == 0
            || width > 0x7fff_ffff
            || height > 0x7fff_ffff
            || data[10] != 0
            || data[11] != 0
            || data[12] > 1
        {
            return Err(Error::Chunk);
        }
        let (depth, color) = (data[8], data[9]);
        let legal = match color {
            0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
            2 | 4 | 6 => matches!(depth, 8 | 16),
            3 => matches!(depth, 1 | 2 | 4 | 8),
            _ => false,
        };
        if !legal {
            return Err(Error::Chunk);
        }
        if depth == 16 {
            return Err(Error::UnsupportedDepth);
        }
        limits.image_len(width, height)?;
        Ok(Self {
            width,
            height,
            depth: depth as usize,
            color,
            channels: match color {
                0 | 3 => 1,
                2 => 3,
                4 => 2,
                6 => 4,
                _ => return Err(Error::Chunk),
            },
            interlace: data[12] == 1,
        })
    }
    fn passes(&self) -> &'static [[usize; 4]] {
        if self.interlace { &ADAM7 } else { &PLAIN }
    }
    fn row_bytes(&self, width: usize) -> usize {
        (width * self.channels * self.depth).div_ceil(8)
    }
    fn expected(&self) -> usize {
        self.passes()
            .iter()
            .map(|&[sx, sy, dx, dy]| {
                let w = pass_size(self.width as usize, sx, dx);
                let h = pass_size(self.height as usize, sy, dy);
                if w == 0 || h == 0 {
                    0
                } else {
                    h * (1 + self.row_bytes(w))
                }
            })
            .sum()
    }
}
fn pass_size(size: usize, start: usize, step: usize) -> usize {
    size.saturating_sub(start).div_ceil(step)
}

fn chunk<'a>(input: &'a [u8], pos: &mut usize) -> Result<([u8; 4], &'a [u8]), Error> {
    let header = input.get(*pos..*pos + 8).ok_or(Error::Chunk)?;
    let len = u32::from_be_bytes(header[..4].try_into().map_err(|_| Error::Chunk)?) as usize;
    if len > 0x7fff_ffff {
        return Err(Error::Chunk);
    }
    let name: [u8; 4] = header[4..].try_into().map_err(|_| Error::Chunk)?;
    if !name.iter().all(u8::is_ascii_alphabetic) {
        return Err(Error::Chunk);
    }
    let end = pos
        .checked_add(12)
        .and_then(|n| n.checked_add(len))
        .ok_or(Error::Chunk)?;
    let all = input.get(*pos..end).ok_or(Error::Chunk)?;
    let data = &all[8..8 + len];
    let expected = u32::from_be_bytes(all[8 + len..].try_into().map_err(|_| Error::Chunk)?);
    if crc32(&[&name, data]) != expected {
        return Err(Error::Crc);
    }
    *pos = end;
    Ok((name, data))
}

/// Decode the documented static profile into straight RGBA8. The input is
/// borrowed; consecutive IDAT payloads are joined only when necessary. All
/// allocations are bounded before inflate; ancillary payloads are never inflated.
pub fn decode(input: &[u8], limits: Limits, work: &mut Work) -> Result<Decoded, Error> {
    if input.len() > MAX_INPUT {
        return Err(crate::Error::Limit(crate::Resource::Bytes).into());
    }
    work.charge(input.len() as u64)?;
    if !input.starts_with(SIGNATURE) {
        return Err(Error::Signature);
    }
    let mut pos = 8;
    let (name, data) = chunk(input, &mut pos)?;
    if &name != b"IHDR" {
        return Err(Error::Chunk);
    }
    let header = Header::parse(data, limits)?;
    let expected = header.expected();
    limits.check_buffer(expected as u64)?;
    let mut compressed = Cow::Borrowed(&[][..]);
    let mut palette = [[0, 0, 0, 255]; 256];
    let mut palette_len = None;
    let mut transparency = None;
    let mut has_transparency = false;
    let mut phase = 0;
    let mut uninterpreted_color = false;
    let mut skipped_ancillary = false;
    loop {
        let (name, data) = chunk(input, &mut pos)?;
        if &name != b"IDAT" && phase == 1 {
            phase = 2;
        }
        match &name {
            b"IHDR" => return Err(Error::Chunk),
            b"PLTE" => {
                if phase != 0
                    || palette_len.is_some()
                    || has_transparency
                    || matches!(header.color, 0 | 4)
                    || data.is_empty()
                    || data.len() % 3 != 0
                    || data.len() > 768
                {
                    return Err(Error::Chunk);
                }
                let count = data.len() / 3;
                if header.color == 3 && count > 1 << header.depth {
                    return Err(Error::Chunk);
                }
                for (target, samples) in palette.iter_mut().zip(data.as_chunks::<3>().0.iter()) {
                    target[..3].copy_from_slice(samples);
                }
                palette_len = Some(count);
            }
            b"tRNS" => {
                if phase != 0 || has_transparency {
                    return Err(Error::Chunk);
                }
                has_transparency = true;
                let mask = (1u16 << header.depth) - 1;
                match header.color {
                    0 if data.len() == 2 => {
                        transparency = Some([u16::from_be_bytes([data[0], data[1]]) & mask, 0, 0])
                    }
                    2 if data.len() == 6 => {
                        transparency = Some([
                            u16::from_be_bytes([data[0], data[1]]) & 255,
                            u16::from_be_bytes([data[2], data[3]]) & 255,
                            u16::from_be_bytes([data[4], data[5]]) & 255,
                        ])
                    }
                    3 if palette_len.is_some_and(|count| data.len() <= count) => {
                        for (target, &alpha) in palette.iter_mut().zip(data) {
                            target[3] = alpha;
                        }
                    }
                    _ => return Err(Error::Chunk),
                }
            }
            b"IDAT" => {
                if phase == 2 || (header.color == 3 && palette_len.is_none()) {
                    return Err(Error::Chunk);
                }
                phase = 1;
                append_idat(&mut compressed, data, limits, work)?;
            }
            b"IEND" => {
                if !data.is_empty() || phase == 0 || pos != input.len() {
                    return Err(Error::Chunk);
                }
                break;
            }
            b"acTL" | b"fcTL" | b"fdAT" => return Err(Error::UnsupportedAnimation),
            b"cHRM" | b"gAMA" | b"iCCP" | b"sBIT" | b"sRGB" | b"cICP" | b"mDCV" | b"cLLI" => {
                uninterpreted_color = true
            }
            _ if name[0] & 32 == 0 => return Err(Error::Unsupported),
            _ => skipped_ancillary = true,
        }
    }
    let inflated = crate::deflate::inflate(&compressed, expected, work)?;
    let trailing_idat_bytes = compressed.len() - inflated.consumed;
    drop(compressed); // Release joined IDAT storage before allocating the RGBA image.
    let image = reconstruct(
        inflated.data,
        &header,
        &palette,
        palette_len.unwrap_or(0),
        transparency,
        limits,
        work,
    )?;
    Ok(Decoded {
        image,
        uninterpreted_color,
        skipped_ancillary,
        trailing_idat_bytes,
    })
}

fn append_idat<'a>(
    out: &mut Cow<'a, [u8]>,
    data: &'a [u8],
    limits: Limits,
    work: &mut Work,
) -> Result<(), Error> {
    limits.check_buffer((out.len() + data.len()) as u64)?;
    match out {
        Cow::Borrowed(previous) if previous.is_empty() => *previous = data,
        Cow::Borrowed(previous) => {
            work.charge((previous.len() + data.len()) as u64)?;
            let mut joined = Vec::new();
            joined
                .try_reserve_exact(previous.len() + data.len())
                .map_err(|_| crate::Error::Allocation)?;
            joined.extend_from_slice(previous);
            joined.extend_from_slice(data);
            *out = Cow::Owned(joined);
        }
        Cow::Owned(joined) => {
            work.charge(data.len() as u64)?;
            joined
                .try_reserve(data.len())
                .map_err(|_| crate::Error::Allocation)?;
            joined.extend_from_slice(data);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reconstruct(
    mut raw: Vec<u8>,
    header: &Header,
    palette: &[[u8; 4]; 256],
    palette_len: usize,
    transparency: Option<[u16; 3]>,
    limits: Limits,
    work: &mut Work,
) -> Result<Image, Error> {
    // Noninterlaced RGBA already has the target byte layout after unfiltering.
    // Remove scanline filter bytes in place and adopt the allocation. Typed pixel
    // views stay safe through slice::as_chunks, without transmute or unsafe code.
    if header.color == 6 && !header.interlace {
        let row_bytes = header.width as usize * 4;
        let stride = row_bytes + 1;
        for y in 0..header.height as usize {
            let offset = y * stride;
            let (before, tail) = raw.split_at_mut(offset);
            let filter = tail[0];
            let row = &mut tail[1..stride];
            let previous = if y == 0 {
                None
            } else {
                Some(&before[offset - row_bytes..offset])
            };
            work.charge((row_bytes + header.width as usize) as u64)?;
            unfilter(row, previous, 4, filter)?;
        }
        for y in 0..header.height as usize {
            let start = y * stride + 1;
            raw.copy_within(start..start + row_bytes, y * row_bytes);
        }
        raw.truncate(header.width as usize * header.height as usize * 4);
        return Ok(Image::from_rgba_bytes(
            header.width,
            header.height,
            raw,
            limits,
            work,
        )?);
    }
    let mut image = Image::new(header.width, header.height, limits, work)?;
    let mut lookup = *palette;
    if header.color == 0 {
        let maximum = (1 << header.depth) - 1;
        for (sample, pixel) in lookup.iter_mut().enumerate().take(maximum + 1) {
            let gray = (sample * 255 / maximum) as u8;
            *pixel = [
                gray,
                gray,
                gray,
                if transparency.is_some_and(|key| key[0] as usize == sample) {
                    0
                } else {
                    255
                },
            ];
        }
    }
    let mut offset = 0;
    let bpp = (header.channels * header.depth).div_ceil(8).max(1);
    for &[sx, sy, dx, dy] in header.passes() {
        let width = pass_size(header.width as usize, sx, dx);
        let height = pass_size(header.height as usize, sy, dy);
        if width == 0 || height == 0 {
            continue;
        }
        let row_bytes = header.row_bytes(width);
        for row_index in 0..height {
            let (before, tail) = raw.split_at_mut(offset);
            let filter = tail[0];
            let row = &mut tail[1..row_bytes + 1];
            let previous = if row_index == 0 {
                None
            } else {
                Some(&before[before.len() - row_bytes..])
            };
            work.charge((row_bytes + width) as u64)?;
            unfilter(row, previous, bpp, filter)?;
            let y = sy + row_index * dy;
            let start = y * header.width as usize;
            let destination = &mut image.pixels_mut()[start..start + header.width as usize];
            write_row(
                row,
                destination.iter_mut().skip(sx).step_by(dx),
                header,
                &lookup,
                palette_len,
                transparency,
            )?;
            offset += row_bytes + 1;
        }
    }
    Ok(image)
}

fn unfilter(row: &mut [u8], previous: Option<&[u8]>, bpp: usize, filter: u8) -> Result<(), Error> {
    match filter {
        0 => {}
        1 => {
            for i in bpp..row.len() {
                row[i] = row[i].wrapping_add(row[i - bpp]);
            }
        }
        2 => {
            if let Some(previous) = previous {
                for (value, &up) in row.iter_mut().zip(previous) {
                    *value = value.wrapping_add(up);
                }
            }
        }
        3 | 4 => {
            for i in 0..row.len() {
                let left = if i >= bpp { row[i - bpp] } else { 0 };
                let up = previous.map_or(0, |p| p[i]);
                let upper_left = if i >= bpp {
                    previous.map_or(0, |p| p[i - bpp])
                } else {
                    0
                };
                let predict = if filter == 3 {
                    ((u16::from(left) + u16::from(up)) / 2) as u8
                } else {
                    paeth(left, up, upper_left)
                };
                row[i] = row[i].wrapping_add(predict);
            }
        }
        _ => return Err(Error::Filter),
    }
    Ok(())
}
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (i32::from(a), i32::from(b), i32::from(c));
    let p = a + b - c;
    let (da, db, dc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if da <= db && da <= dc {
        a as u8
    } else if db <= dc {
        b as u8
    } else {
        c as u8
    }
}

fn write_row<'a>(
    row: &[u8],
    destination: impl Iterator<Item = &'a mut [u8; 4]>,
    header: &Header,
    lookup: &[[u8; 4]; 256],
    palette_len: usize,
    transparency: Option<[u16; 3]>,
) -> Result<(), Error> {
    match header.color {
        0 | 3 => {
            let mask = (1usize << header.depth) - 1;
            for (index, pixel) in destination.enumerate() {
                let bit = index * header.depth;
                let sample = (row[bit / 8] as usize >> (8 - header.depth - bit % 8)) & mask;
                if header.color == 3 && sample >= palette_len {
                    return Err(Error::Chunk);
                }
                *pixel = lookup[sample];
            }
        }
        2 => {
            for (pixel, samples) in destination.zip(row.as_chunks::<3>().0.iter()) {
                let key = [
                    u16::from(samples[0]),
                    u16::from(samples[1]),
                    u16::from(samples[2]),
                ];
                *pixel = [
                    samples[0],
                    samples[1],
                    samples[2],
                    if transparency == Some(key) { 0 } else { 255 },
                ];
            }
        }
        4 => {
            for (pixel, samples) in destination.zip(row.as_chunks::<2>().0.iter()) {
                *pixel = [samples[0], samples[0], samples[0], samples[1]];
            }
        }
        6 => {
            for (pixel, samples) in destination.zip(row.as_chunks::<4>().0.iter()) {
                pixel.copy_from_slice(samples);
            }
        }
        _ => return Err(Error::Chunk),
    }
    Ok(())
}

fn write_chunk(output: &mut impl std::io::Write, name: &[u8; 4], data: &[u8]) -> Result<(), Error> {
    output.write_all(&(data.len() as u32).to_be_bytes())?;
    output.write_all(name)?;
    output.write_all(data)?;
    output.write_all(&crc32(&[name, data]).to_be_bytes())?;
    Ok(())
}

fn write_header(image: &Image, output: &mut impl std::io::Write) -> Result<(), Error> {
    let mut header = [0u8; 13];
    header[..4].copy_from_slice(&image.width.to_be_bytes());
    header[4..8].copy_from_slice(&image.height.to_be_bytes());
    header[8] = 8;
    header[9] = 6;
    output.write_all(SIGNATURE)?;
    write_chunk(output, b"IHDR", &header)?;
    Ok(())
}

/// Stream deterministic noninterlaced RGBA8, filter None and stored DEFLATE.
/// Scratch is at most 65,540 bytes. Work is preflighted before any output;
/// I/O errors may leave partial bytes in the caller's sink. File publication
/// and buffering belong to the caller, not the codec.
pub fn encode(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
) -> Result<(), Error> {
    let raw_size = (image.width as usize * 4 + 1) * image.height as usize;
    let blocks = raw_size.div_ceil(65_535);
    let encoded_size = 75 + raw_size + blocks * 17;
    work.charge((raw_size * 3 + encoded_size) as u64)?;
    let mut block = Vec::new();
    block
        .try_reserve_exact(raw_size.min(65_535) + 5)
        .map_err(|_| crate::Error::Allocation)?;
    write_header(image, output)?;
    write_chunk(output, b"IDAT", &[0x78, 0x01])?;
    let rgba = image.pixels().as_flattened();
    let row_bytes = image.width as usize * 4;
    let stride = row_bytes + 1;
    let mut raw_offset = 0;
    let mut remaining = raw_size;
    let mut adler = Adler::new();
    while remaining > 0 {
        let count = remaining.min(65_535);
        block.clear();
        block.push(u8::from(count == remaining));
        block.extend_from_slice(&(count as u16).to_le_bytes());
        block.extend_from_slice(&(!(count as u16)).to_le_bytes());
        while block.len() < count + 5 {
            let row = raw_offset / stride;
            let column = raw_offset % stride;
            if column == 0 {
                block.push(0);
                raw_offset += 1;
            } else {
                let take = (stride - column).min(count + 5 - block.len());
                let start = row * row_bytes + column - 1;
                block.extend_from_slice(&rgba[start..start + take]);
                raw_offset += take;
            }
        }
        adler.update(&block[5..]);
        write_chunk(output, b"IDAT", &block)?;
        remaining -= count;
    }
    write_chunk(output, b"IDAT", &adler.value().to_be_bytes())?;
    write_chunk(output, b"IEND", &[])
}

/// Lossless RGBA8 output with Sub on the first row, Up on subsequent rows,
/// bounded greedy matching and fixed Huffman codes (stored fallback for noise).
/// This changes PNG bytes, never RGBA samples. The original `encode` retains its
/// exact byte contract. Scratch is below 169 KiB, independent of image size.
/// Work/allocation preflight precedes output; I/O errors may leave partial bytes.
/// See benchmarks/png/algorithm-notes.md for algorithm sources and credits.
pub fn encode_compressed(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
) -> Result<(), Error> {
    let row_bytes = image.width as usize * 4;
    let stride = row_bytes + 1;
    let raw_size = stride * image.height as usize;
    let blocks = raw_size.div_ceil(crate::deflate::BLOCK);
    // A linear budget for filtering, bounded matching, coding and checksums,
    // plus the fixed hash-table reset for each block. No search chains exist.
    work.charge((raw_size * 12 + blocks * (8192 + 64)) as u64)?;
    let capacity = raw_size.min(crate::deflate::BLOCK);
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity)
        .map_err(|_| crate::Error::Allocation)?;
    let mut encoder = crate::deflate::Encoder::new(capacity)?;
    write_header(image, output)?;
    write_chunk(output, b"IDAT", &[0x78, 0x01])?;
    let rgba = image.pixels().as_flattened();
    let mut offset = 0;
    let mut adler = Adler::new();
    while offset < raw_size {
        let count = (raw_size - offset).min(capacity);
        raw.resize(count, 0);
        let mut filled = 0;
        while filled < count {
            let row = offset / stride;
            let column = offset % stride;
            if column == 0 {
                raw[filled] = if row == 0 { 1 } else { 2 };
                offset += 1;
                filled += 1;
            } else {
                let take = (stride - column).min(count - filled);
                let start = row * row_bytes + column - 1;
                let destination = &mut raw[filled..filled + take];
                if row == 0 {
                    for (i, value) in destination.iter_mut().enumerate() {
                        let x = start + i;
                        *value = rgba[x].wrapping_sub(if x < 4 { 0 } else { rgba[x - 4] });
                    }
                } else {
                    for ((value, &sample), &up) in destination
                        .iter_mut()
                        .zip(&rgba[start..start + take])
                        .zip(&rgba[start - row_bytes..start - row_bytes + take])
                    {
                        *value = sample.wrapping_sub(up);
                    }
                }
                offset += take;
                filled += take;
            }
        }
        adler.update(&raw);
        write_chunk(output, b"IDAT", encoder.block(&raw, offset == raw_size))?;
    }
    write_chunk(output, b"IDAT", &adler.value().to_be_bytes())?;
    write_chunk(output, b"IEND", &[])
}
