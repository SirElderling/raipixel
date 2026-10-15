//! Bounded greedy LZ matching and fixed Huffman output, authored from RFC 1951.
//! Research context/credits: benchmarks/png/algorithm-notes.md. Bille, Cording,
//! Fischer and Gørtz (CPM 2017) inform the bounded-window design discussion;
//! this is a hash matcher, not their exact suffix-tree parsing algorithm.
use super::{DISTANCE_BASE, DISTANCE_EXTRA, LENGTH_BASE, LENGTH_EXTRA};
use crate::Error;

pub(crate) const BLOCK: usize = 65_535;
const HASH_SIZE: usize = 8192;

pub(crate) struct Encoder {
    positions: Vec<u32>,
    output: Vec<u8>,
    bits: u32,
    count: u32,
}
fn hash(data: &[u8], pos: usize) -> usize {
    let key = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], 0]);
    (key.wrapping_mul(0x9e37_79b1) >> 19) as usize
}

fn match_length(data: &[u8], prior: usize, pos: usize, maximum: usize) -> usize {
    if prior >= pos || pos - prior > 32_768 || data[prior..prior + 3] != data[pos..pos + 3] {
        return 0;
    }
    let mut length = 3;
    while length + 8 <= maximum
        && data[prior + length..prior + length + 8] == data[pos + length..pos + length + 8]
    {
        length += 8;
    }
    while length < maximum && data[prior + length] == data[pos + length] {
        length += 1;
    }
    length
}

impl Encoder {
    pub(crate) fn new(max_block: usize) -> Result<Self, Error> {
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(HASH_SIZE)
            .map_err(|_| Error::Allocation)?;
        positions.resize(HASH_SIZE, u32::MAX);
        let mut output = Vec::new();
        // At most nine bits per literal, plus headers, EOB and pending bits.
        output
            .try_reserve_exact(max_block * 9 / 8 + 16)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            positions,
            output,
            bits: 0,
            count: 0,
        })
    }

    fn put(&mut self, value: u32, count: u32) {
        self.bits |= value << self.count;
        self.count += count;
        while self.count >= 8 {
            self.output.push(self.bits as u8);
            self.bits >>= 8;
            self.count -= 8;
        }
    }
    fn align(&mut self) {
        if self.count != 0 {
            self.put(0, 8 - self.count);
        }
    }
    fn symbol(&mut self, symbol: u32) {
        let (code, count) = match symbol {
            0..=143 => (symbol + 48, 8),
            144..=255 => (symbol - 144 + 400, 9),
            256..=279 => (symbol - 256, 7),
            280..=285 => (symbol - 280 + 192, 8),
            _ => unreachable!("encoder symbol outside RFC 1951 alphabet"),
        };
        self.put(code.reverse_bits() >> (32 - count), count);
    }
    fn reference(&mut self, length: usize, distance: usize) {
        let index = LENGTH_BASE.partition_point(|&base| base <= length) - 1;
        self.symbol(257 + index as u32);
        self.put((length - LENGTH_BASE[index]) as u32, LENGTH_EXTRA[index]);
        let index = DISTANCE_BASE.partition_point(|&base| base <= distance) - 1;
        self.put((index as u32).reverse_bits() >> 27, 5);
        self.put(
            (distance - DISTANCE_BASE[index]) as u32,
            DISTANCE_EXTRA[index],
        );
    }

    /// Block-local dictionary, with no match search chains or input-sized index.
    /// Fixed blocks keep bit continuity; only stored blocks/final output align.
    pub(crate) fn block(&mut self, data: &[u8], last: bool) -> &[u8] {
        debug_assert!(data.len() <= BLOCK);
        let saved = (self.bits, self.count);
        self.output.clear();
        self.positions.fill(u32::MAX);
        self.put(u32::from(last) | 2, 3); // BTYPE=01, least-significant bit first.
        let mut pos = 0;
        while pos + 3 <= data.len() {
            let slot = hash(data, pos);
            let mut prior = self.positions[slot] as usize;
            self.positions[slot] = pos as u32;
            let maximum = (data.len() - pos).min(258);
            let mut length = match_length(data, prior, pos, maximum);
            // A second, constant-time candidate favors distance-one runs without
            // replacing useful dictionary entries or following a search chain.
            if pos > 0 && data[pos] == data[pos - 1] {
                let repeated = match_length(data, pos - 1, pos, maximum);
                if repeated >= 3 && repeated >= length {
                    prior = pos - 1;
                    length = repeated;
                }
            }
            if length >= 3 {
                self.reference(length, pos - prior);
                pos += length;
            } else {
                self.symbol(u32::from(data[pos]));
                pos += 1;
            }
        }
        for &byte in &data[pos..] {
            self.symbol(u32::from(byte));
        }
        self.symbol(256);
        if last {
            self.align();
        }
        // A stored block is preferable when fixed codes expand this input.
        // Restore the pending bits from the preceding block before retrying.
        if self.output.len() > data.len() + 5 {
            (self.bits, self.count) = saved;
            self.output.clear();
            self.put(u32::from(last), 3);
            self.align();
            self.output
                .extend_from_slice(&(data.len() as u16).to_le_bytes());
            self.output
                .extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
            self.output.extend_from_slice(data);
        }
        &self.output
    }
}
