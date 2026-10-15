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

fn distance_code(distance: usize) -> usize {
    let x = distance - 1;
    if x < 4 {
        x
    } else {
        let n = x.ilog2() as usize;
        2 * n + ((x >> (n - 1)) & 1)
    }
}

const fn fixed_codes() -> [(u16, u8); 286] {
    let mut result = [(0, 0); 286];
    let mut symbol = 0;
    while symbol < result.len() {
        let (code, count) = match symbol {
            0..=143 => (symbol as u16 + 48, 8),
            144..=255 => (symbol as u16 - 144 + 400, 9),
            256..=279 => (symbol as u16 - 256, 7),
            _ => (symbol as u16 - 280 + 192, 8),
        };
        result[symbol] = (code.reverse_bits() >> (16 - count), count);
        symbol += 1;
    }
    result
}
const fn length_codes() -> [u8; 256] {
    let mut result = [0; 256];
    let mut length = 3;
    let mut index = 0;
    while length <= 258 {
        while index + 1 < LENGTH_BASE.len() && LENGTH_BASE[index + 1] <= length {
            index += 1;
        }
        result[length - 3] = index as u8;
        length += 1;
    }
    result
}
static FIXED_CODES: [(u16, u8); 286] = fixed_codes();
static LENGTH_CODES: [u8; 256] = length_codes();

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
        let (code, count) = FIXED_CODES[symbol as usize];
        self.put(u32::from(code), u32::from(count));
    }
    fn reference(&mut self, length: usize, distance: usize) {
        let index = usize::from(LENGTH_CODES[length - 3]);
        self.symbol(257 + index as u32);
        self.put((length - LENGTH_BASE[index]) as u32, LENGTH_EXTRA[index]);
        let index = distance_code(distance);
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
            let stored = self.positions[slot] as usize;
            self.positions[slot] = pos as u32;
            let maximum = (data.len() - pos).min(258);
            let repeated = if pos > 0 && data[pos] == data[pos - 1] {
                match_length(data, pos - 1, pos, maximum)
            } else {
                0
            };
            // The old tie-break already preferred distance one. A maximal run
            // cannot lose; retain the dictionary update for subsequent tokens.
            let (prior, length) = if repeated == 0 {
                (stored, match_length(data, stored, pos, maximum))
            } else if repeated == maximum {
                (pos - 1, repeated)
            } else {
                let matched = match_length(data, stored, pos, maximum);
                if repeated >= matched {
                    (pos - 1, repeated)
                } else {
                    (stored, matched)
                }
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_symbols_match_rfc_codes_at_every_pending_offset() {
        let mut encoder = Encoder::new(16).unwrap();
        for symbol in 0..=285u32 {
            let (code, count) = match symbol {
                0..=143 => (symbol + 48, 8),
                144..=255 => (symbol - 144 + 400, 9),
                256..=279 => (symbol - 256, 7),
                _ => (symbol - 280 + 192, 8),
            };
            let reversed = code.reverse_bits() >> (32 - count);
            for pending in 0..8 {
                encoder.output.clear();
                encoder.bits = (1 << pending) - 1;
                encoder.count = pending;
                encoder.symbol(symbol);
                let bits = ((1u32 << pending) - 1) | (reversed << pending);
                let total = count + pending;
                let bytes = (total / 8) as usize;
                assert_eq!(encoder.output, bits.to_le_bytes()[..bytes]);
                assert_eq!(encoder.bits, bits >> (bytes * 8));
                assert_eq!(encoder.count, total % 8);
            }
        }
    }

    #[test]
    fn matches_find_every_termination_byte_and_overlapping_run() {
        for maximum in 3..=258 {
            for mismatch in 3..maximum {
                let mut data = vec![19; 2 * 258];
                data[258 + mismatch] = 37;
                assert_eq!(match_length(&data, 0, 258, maximum), mismatch);
            }
            assert_eq!(match_length(&[19; 259], 0, 1, maximum), maximum);
        }
    }

    #[test]
    fn references_preserve_rfc_bits_at_every_length_and_pending_offset() {
        let mut actual = Encoder::new(32).unwrap();
        let mut expected = Encoder::new(32).unwrap();
        for length in 3..=258 {
            for (index, &base) in DISTANCE_BASE.iter().enumerate() {
                for distance in [base, base + (1 << DISTANCE_EXTRA[index]) - 1] {
                    for pending in 0..8 {
                        actual.output.clear();
                        expected.output.clear();
                        actual.bits = 0;
                        actual.count = 0;
                        expected.bits = 0;
                        expected.count = 0;
                        actual.put(0x55 & ((1 << pending) - 1), pending);
                        expected.put(0x55 & ((1 << pending) - 1), pending);
                        actual.reference(length, distance);
                        let index = LENGTH_BASE.partition_point(|&b| b <= length) - 1;
                        expected.symbol(257 + index as u32);
                        expected.put((length - LENGTH_BASE[index]) as u32, LENGTH_EXTRA[index]);
                        let index = DISTANCE_BASE.partition_point(|&b| b <= distance) - 1;
                        expected.put((index as u32).reverse_bits() >> 27, 5);
                        expected.put(
                            (distance - DISTANCE_BASE[index]) as u32,
                            DISTANCE_EXTRA[index],
                        );
                        assert_eq!(
                            (&actual.output, actual.bits, actual.count),
                            (&expected.output, expected.bits, expected.count),
                            "length {length}, distance {distance}, pending {pending}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn distance_codes_match_every_rfc_range() {
        for distance in 1..=32_768 {
            let expected = DISTANCE_BASE.partition_point(|&base| base <= distance) - 1;
            assert_eq!(distance_code(distance), expected, "distance {distance}");
            assert!(distance - DISTANCE_BASE[expected] < 1 << DISTANCE_EXTRA[expected]);
        }
    }
}
