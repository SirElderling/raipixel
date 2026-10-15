//! Private zlib/DEFLATE decoder; no PNG chunk or pixel knowledge.
use crate::{Error as ResourceError, Work, checksum::Adler};
use std::borrow::Cow;
mod encode;
pub(crate) use encode::{BLOCK, Encoder};

#[derive(Debug)]
pub(crate) enum Error {
    Data,
    Adler,
    Resource(ResourceError),
}
impl From<ResourceError> for Error {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}

pub(crate) struct Inflated {
    pub data: Vec<u8>,
    pub consumed: usize,
}

struct Bits<'a, 'w> {
    data: &'a [u8],
    pos: usize,
    buffer: u64,
    count: u32,
    work: &'w mut Work,
}
impl Bits<'_, '_> {
    fn fill(&mut self, count: u32) -> Result<(), Error> {
        while self.count < count && self.pos < self.data.len() {
            self.work.charge(1)?;
            self.buffer |= u64::from(self.data[self.pos]) << self.count;
            self.count += 8;
            self.pos += 1;
        }
        Ok(())
    }
    fn read(&mut self, count: u32) -> Result<u32, Error> {
        self.fill(count)?;
        if self.count < count {
            return Err(Error::Data);
        }
        let value = (self.buffer & ((1u64 << count) - 1)) as u32;
        self.consume(count);
        Ok(value)
    }
    fn consume(&mut self, count: u32) {
        self.buffer >>= count;
        self.count -= count;
    }
    fn align(&mut self) {
        self.consume(self.count % 8);
    }
}

#[derive(Clone, Copy, Default)]
struct Entry {
    symbol: u16,
    bits: u8,
}
#[derive(Clone, Copy)]
enum Alphabet {
    Codes,
    Literals,
    Distances,
}
struct Huffman {
    entries: Cow<'static, [Entry]>,
    width: u32,
}
impl Huffman {
    fn new(lengths: &[u8], alphabet: Alphabet, work: &mut Work) -> Result<Self, Error> {
        let mut counts = [0i32; 16];
        let mut width = 0;
        for &len in lengths {
            if len > 15 {
                return Err(Error::Data);
            }
            if len != 0 {
                counts[len as usize] += 1;
                width = width.max(u32::from(len));
            }
        }
        if width == 0 {
            return if matches!(alphabet, Alphabet::Distances) {
                Ok(Self {
                    entries: Cow::Borrowed(&[]),
                    width: 0,
                })
            } else {
                Err(Error::Data)
            };
        }
        let mut left = 1i32;
        for &count in &counts[1..] {
            left = left * 2 - count;
            if left < 0 {
                return Err(Error::Data);
            }
        }
        // RFC 1951 permits one one-bit code for a degenerate data alphabet;
        // code-length alphabets must be complete. Empty distance trees may
        // represent literal-only blocks but can never decode a distance.
        if left != 0 && (matches!(alphabet, Alphabet::Codes) || width != 1 || counts[1] != 1) {
            return Err(Error::Data);
        }
        let size = 1usize << width;
        work.charge((size + lengths.len()) as u64)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(size)
            .map_err(|_| Error::Resource(ResourceError::Allocation))?;
        entries.resize(size, Entry::default());
        let mut next = [0u16; 16];
        for bits in 1..16 {
            next[bits] = (next[bits - 1] + counts[bits - 1] as u16) << 1;
        }
        for (symbol, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let code = next[len as usize];
            next[len as usize] += 1;
            let reversed = (code.reverse_bits() >> (16 - len)) as usize;
            for index in (reversed..size).step_by(1usize << len) {
                entries[index] = Entry {
                    symbol: symbol as u16,
                    bits: len,
                };
            }
        }
        Ok(Self {
            entries: Cow::Owned(entries),
            width,
        })
    }
    fn symbol(&self, bits: &mut Bits<'_, '_>) -> Result<u16, Error> {
        if self.width == 0 {
            return Err(Error::Data);
        }
        bits.fill(self.width)?;
        let entry = self.entries[(bits.buffer & ((1 << self.width) - 1)) as usize];
        if entry.bits == 0 || u32::from(entry.bits) > bits.count {
            return Err(Error::Data);
        }
        bits.work.charge(1)?;
        bits.consume(u32::from(entry.bits));
        Ok(entry.symbol)
    }
}

// The fixed alphabets are defined by RFC 1951. Construct their lookup tables
// once at compile time, avoiding per-block allocation and reconstruction.
const fn fixed_literals() -> [Entry; 512] {
    let mut entries = [Entry { symbol: 0, bits: 0 }; 512];
    let mut symbol = 0u16;
    while symbol < 288 {
        let (code, bits) = if symbol < 144 {
            (symbol + 48, 8u8)
        } else if symbol < 256 {
            (symbol - 144 + 400, 9)
        } else if symbol < 280 {
            (symbol - 256, 7)
        } else {
            (symbol - 280 + 192, 8)
        };
        let mut index = (code.reverse_bits() >> (16 - bits)) as usize;
        while index < 512 {
            entries[index] = Entry { symbol, bits };
            index += 1usize << bits;
        }
        symbol += 1;
    }
    entries
}
const fn fixed_distances() -> [Entry; 32] {
    let mut entries = [Entry { symbol: 0, bits: 5 }; 32];
    let mut symbol = 0u16;
    while symbol < 32 {
        entries[(symbol.reverse_bits() >> 11) as usize].symbol = symbol;
        symbol += 1;
    }
    entries
}
static FIXED_LITERALS: [Entry; 512] = fixed_literals();
static FIXED_DISTANCES: [Entry; 32] = fixed_distances();
fn fixed() -> (Huffman, Huffman) {
    (
        Huffman {
            entries: Cow::Borrowed(&FIXED_LITERALS),
            width: 9,
        },
        Huffman {
            entries: Cow::Borrowed(&FIXED_DISTANCES),
            width: 5,
        },
    )
}

fn dynamic(bits: &mut Bits<'_, '_>) -> Result<(Huffman, Huffman), Error> {
    let literal_count = bits.read(5)? as usize + 257;
    let distance_count = bits.read(5)? as usize + 1;
    let code_count = bits.read(4)? as usize + 4;
    if literal_count > 286 {
        return Err(Error::Data);
    }
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut code_lengths = [0; 19];
    for &index in &ORDER[..code_count] {
        code_lengths[index] = bits.read(3)? as u8;
    }
    let codes = Huffman::new(&code_lengths, Alphabet::Codes, bits.work)?;
    let total = literal_count + distance_count;
    let mut lengths = [0u8; 318];
    let mut index = 0;
    while index < total {
        let symbol = codes.symbol(bits)?;
        let (value, count) = match symbol {
            0..=15 => (symbol as u8, 1),
            16 => {
                if index == 0 {
                    return Err(Error::Data);
                }
                (lengths[index - 1], bits.read(2)? as usize + 3)
            }
            17 => (0, bits.read(3)? as usize + 3),
            18 => (0, bits.read(7)? as usize + 11),
            _ => return Err(Error::Data),
        };
        if count > total - index {
            return Err(Error::Data);
        }
        bits.work.charge(count as u64)?;
        lengths[index..index + count].fill(value);
        index += count;
    }
    if lengths[256] == 0 {
        return Err(Error::Data);
    }
    Ok((
        Huffman::new(&lengths[..literal_count], Alphabet::Literals, bits.work)?,
        Huffman::new(
            &lengths[literal_count..total],
            Alphabet::Distances,
            bits.work,
        )?,
    ))
}

const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Decode one zlib stream, retaining its consumed byte count for the PNG
/// trailing-IDAT policy. The caller precomputes and bounds exact output size.
pub(crate) fn inflate(input: &[u8], expected: usize, work: &mut Work) -> Result<Inflated, Error> {
    if input.len() < 2 {
        return Err(Error::Data);
    }
    let (cmf, flg) = (input[0], input[1]);
    if cmf & 15 != 8
        || cmf >> 4 > 7
        || (u16::from(cmf) * 256 + u16::from(flg)) % 31 != 0
        || flg & 32 != 0
    {
        return Err(Error::Data);
    }
    let window = 1usize << ((cmf >> 4) + 8);
    let mut out = Vec::new();
    out.try_reserve_exact(expected)
        .map_err(|_| Error::Resource(ResourceError::Allocation))?;
    let mut bits = Bits {
        data: &input[2..],
        pos: 0,
        buffer: 0,
        count: 0,
        work,
    };
    loop {
        let final_block = bits.read(1)? != 0;
        match bits.read(2)? {
            0 => {
                bits.align();
                let len = bits.read(16)? as usize;
                let inverse = bits.read(16)? as u16;
                if (len as u16) ^ inverse != u16::MAX
                    || len > expected - out.len()
                    || bits.count != 0
                {
                    return Err(Error::Data);
                }
                let block = bits.data.get(bits.pos..bits.pos + len).ok_or(Error::Data)?;
                bits.work.charge(2 * len as u64)?;
                out.extend_from_slice(block);
                bits.pos += len;
            }
            kind @ (1 | 2) => {
                let (literals, distances) = if kind == 1 {
                    fixed()
                } else {
                    dynamic(&mut bits)?
                };
                loop {
                    let symbol = literals.symbol(&mut bits)?;
                    match symbol {
                        0..=255 => {
                            if out.len() == expected {
                                return Err(Error::Data);
                            }
                            bits.work.charge(1)?;
                            out.push(symbol as u8);
                        }
                        256 => break,
                        257..=285 => {
                            let index = (symbol - 257) as usize;
                            let len = LENGTH_BASE[index] + bits.read(LENGTH_EXTRA[index])? as usize;
                            let distance_symbol = distances.symbol(&mut bits)? as usize;
                            if distance_symbol >= 30 {
                                return Err(Error::Data);
                            }
                            let distance = DISTANCE_BASE[distance_symbol]
                                + bits.read(DISTANCE_EXTRA[distance_symbol])? as usize;
                            if distance > out.len()
                                || distance > window
                                || len > expected - out.len()
                            {
                                return Err(Error::Data);
                            }
                            bits.work.charge(len as u64)?;
                            let start = out.len() - distance;
                            let mut remaining = len;
                            while remaining > 0 {
                                let count = (out.len() - start).min(remaining);
                                out.extend_from_within(start..start + count);
                                remaining -= count;
                            }
                        }
                        _ => return Err(Error::Data),
                    }
                }
            }
            _ => return Err(Error::Data),
        }
        if final_block {
            break;
        }
    }
    if out.len() != expected {
        return Err(Error::Data);
    }
    bits.align();
    let mut checksum = 0;
    for _ in 0..4 {
        checksum = (checksum << 8) | bits.read(8)?;
    }
    bits.work.charge(out.len() as u64)?;
    let mut adler = Adler::new();
    adler.update(&out);
    if adler.value() != checksum {
        return Err(Error::Adler);
    }
    Ok(Inflated {
        data: out,
        consumed: 2 + bits.pos - bits.count as usize / 8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{self, Value};
    #[test]
    fn independent_stored_fixed_dynamic_and_overlapping_streams() {
        let Value::Array(cases) = json::parse(
            include_bytes!("../tests/fixtures/deflate.json"),
            json::Limits::default(),
            &mut Work::default(),
        )
        .unwrap() else {
            panic!("array expected")
        };
        for case in cases {
            let Value::Object(fields) = case else {
                panic!("object expected")
            };
            let decode = |key: &str| {
                let Value::String(text) = &fields[key] else {
                    panic!("hex expected")
                };
                text.as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect::<Vec<_>>()
            };
            let input = decode("zlib");
            let expected = decode("data");
            let result = inflate(&input, expected.len(), &mut Work::default()).unwrap();
            assert_eq!(result.data, expected);
            assert_eq!(result.consumed, input.len());
        }
    }
    #[test]
    fn rejects_invalid_huffman_trees_and_accepts_defined_degenerate_trees() {
        let mut work = Work::default();
        assert!(Huffman::new(&[1, 1, 1], Alphabet::Literals, &mut work).is_err());
        assert!(Huffman::new(&[2, 2], Alphabet::Literals, &mut work).is_err());
        assert!(Huffman::new(&[1], Alphabet::Codes, &mut work).is_err());
        assert!(Huffman::new(&[0], Alphabet::Literals, &mut work).is_err());
        assert!(Huffman::new(&[0], Alphabet::Distances, &mut work).is_ok());
        assert!(Huffman::new(&[1], Alphabet::Distances, &mut work).is_ok());
        assert!(Huffman::new(&[1], Alphabet::Literals, &mut work).is_ok());
    }
}
