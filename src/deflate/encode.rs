//! Bounded greedy LZ matching and fixed Huffman output, authored from RFC 1951.
//! Research context/credits: benchmarks/png/algorithm-notes.md. Bille, Cording,
//! Fischer and Gørtz (CPM 2017) inform the bounded-window design discussion;
//! this is a hash matcher, not their exact suffix-tree parsing algorithm.
use super::{DISTANCE_BASE, DISTANCE_EXTRA, LENGTH_BASE, LENGTH_EXTRA};
use crate::Error;

pub(crate) const BLOCK: usize = 65_535;
const HASH_SIZE: usize = 8192;

pub(crate) struct Encoder {
    dictionary: Vec<u32>,
    output: Vec<u8>,
    bits: u64,
    count: u32,
}
fn hash(data: &[u8], pos: usize) -> (usize, u32) {
    let key = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], 0]);
    ((key.wrapping_mul(0x9e37_79b1) >> 19) as usize, key & 65535)
}

// Keep candidate rejection inline without also inlining the larger scan.
#[inline(always)]
fn match_candidate(data: &[u8], entry: u32, prefix: u32, pos: usize, maximum: usize) -> usize {
    let prior = (entry & 65535) as usize;
    // The upper half caches the first two bytes of the immutable input. Block
    // positions fit the lower half; 65535 is an unused sentinel. On a prefix hit
    // only the third byte remains to establish the minimum three-byte match.
    if entry >> 16 != prefix
        || prior >= pos
        || pos - prior > 32_768
        || data[prior + 2] != data[pos + 2]
    {
        return 0;
    }
    3 + matching_tail(
        &data[prior + 3..prior + maximum],
        &data[pos + 3..pos + maximum],
    )
}
#[inline(never)]
fn matching_tail(left: &[u8], right: &[u8]) -> usize {
    let mut matched = 0;
    for (a, b) in left
        .as_chunks::<16>()
        .0
        .iter()
        .zip(right.as_chunks::<16>().0)
    {
        if a != b {
            break;
        }
        matched += 16;
    }
    matched
        + left[matched..]
            .iter()
            .zip(&right[matched..])
            .take_while(|(x, y)| x == y)
            .count()
}

#[inline(never)]
fn repeated_length(data: &[u8], prior: usize, pos: usize, maximum: usize) -> usize {
    let byte = data[prior];
    let view = &data[pos..pos + maximum];
    let mut matched = 0;
    for group in view.as_chunks::<16>().0 {
        if *group != [byte; 16] {
            break;
        }
        matched += 16;
    }
    let length = matched + view[matched..].iter().take_while(|&&x| x == byte).count();
    if length < 3 { 0 } else { length }
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

const fn fixed_codes() -> [u16; 286] {
    let mut result = [0; 286];
    let mut symbol = 0;
    while symbol < result.len() {
        let (code, count) = match symbol {
            0..=143 => (symbol as u16 + 48, 8),
            144..=255 => (symbol as u16 - 144 + 400, 9),
            256..=279 => (symbol as u16 - 256, 7),
            _ => (symbol as u16 - 280 + 192, 8),
        };
        result[symbol] = (code.reverse_bits() >> (16 - count)) | ((count as u16) << 9);
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
// Nine low bits hold the reversed code; the high bits hold its bit count.
static FIXED_CODES: [u16; 286] = fixed_codes();
static LENGTH_CODES: [u8; 256] = length_codes();

impl Encoder {
    pub(crate) fn new(max_block: usize) -> Result<Self, Error> {
        let mut dictionary = Vec::new();
        dictionary
            .try_reserve_exact(HASH_SIZE)
            .map_err(|_| Error::Allocation)?;
        dictionary.resize(HASH_SIZE, u32::MAX);
        let mut output = Vec::new();
        // At most nine bits per literal, plus headers, EOB and pending bits.
        output
            .try_reserve_exact(max_block * 9 / 8 + 16)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            dictionary,
            output,
            bits: 0,
            count: 0,
        })
    }

    fn put(&mut self, value: u32, count: u32) {
        // Pending bits stay below 32; the largest write is 13 distance bits.
        // A u64 holds both, and at most one four-byte flush is needed.
        self.bits |= u64::from(value) << self.count;
        self.count += count;
        if self.count >= 32 {
            self.output
                .extend_from_slice(&(self.bits as u32).to_le_bytes());
            self.bits >>= 32;
            self.count -= 32;
        }
    }
    fn flush_bytes(&mut self) {
        let count = self.count / 8;
        self.output
            .extend_from_slice(&self.bits.to_le_bytes()[..count as usize]);
        self.bits >>= count * 8;
        self.count -= count * 8;
    }
    fn align(&mut self) {
        let pending = self.count % 8;
        if pending != 0 {
            self.put(0, 8 - pending);
        }
        self.flush_bytes();
    }
    fn symbol(&mut self, symbol: u32) {
        let packed = FIXED_CODES[symbol as usize];
        self.put(u32::from(packed & 511), u32::from(packed >> 9));
    }
    fn reference(&mut self, length: usize, distance: usize) {
        if length == 258 {
            self.symbol(285);
        } else {
            let index = usize::from(LENGTH_CODES[length - 3]);
            self.symbol(257 + index as u32);
            self.put((length - LENGTH_BASE[index]) as u32, LENGTH_EXTRA[index]);
        }
        let index = distance_code(distance);
        self.put((index as u32).reverse_bits() >> 27, 5);
        self.put(
            (distance - DISTANCE_BASE[index]) as u32,
            DISTANCE_EXTRA[index],
        );
    }

    // Incompressible blocks are often expanded by fixed Huffman coding and
    // then discarded for the stored fallback. Sample across the block before
    // doing that expensive full parse. Require multiple actual three-byte
    // matches to reject the fast path; a single hash collision is insufficient.
    // This is a compression policy heuristic, not a correctness requirement.
    fn probably_stored(data: &[u8]) -> bool {
        // Keep the 1 Ki-slot sample table independent of the actual matcher.
        // No extra full dictionary reset on compressible input, and no reset
        // at all when we immediately emit a stored block.
        let mut sampled = [u32::MAX; 1024];
        let mut matches = 0;
        for window in 0..8 {
            let offset = window * (data.len() - 258) / 7;
            for pos in offset..offset + 256 {
                let (slot, prefix) = hash(data, pos);
                let slot = slot & (sampled.len() - 1);
                let entry = sampled[slot];
                sampled[slot] = (prefix << 16) | pos as u32;
                let prior = (entry & 65_535) as usize;
                if entry >> 16 == prefix
                    && prior < pos
                    && pos - prior <= 32_768
                    && data[prior + 2] == data[pos + 2]
                {
                    matches += 1;
                    if matches >= 4 {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn store(&mut self, data: &[u8], last: bool) {
        self.output.clear();
        self.put(u32::from(last), 3);
        self.align();
        self.output
            .extend_from_slice(&(data.len() as u16).to_le_bytes());
        self.output
            .extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
        self.output.extend_from_slice(data);
    }

    /// Block-local dictionary, with no match search chains or input-sized index.
    /// Fixed blocks keep bit continuity; only stored blocks/final output align.
    pub(crate) fn block(&mut self, data: &[u8], last: bool) -> &[u8] {
        self.block_with::<false>(data, last)
    }

    /// A size-oriented parse using one lookahead per non-maximal match.
    /// The fast writer retains its existing bitstream and work.
    pub(crate) fn block_compact(&mut self, data: &[u8], last: bool) -> &[u8] {
        self.block_with::<true>(data, last)
    }

    fn block_with<const COMPACT: bool>(&mut self, data: &[u8], last: bool) -> &[u8] {
        debug_assert!(data.len() <= BLOCK);
        let saved = (self.bits, self.count);
        self.output.clear();
        if data.len() >= 16_384 && Self::probably_stored(data) {
            self.store(data, last);
            return &self.output;
        }
        self.dictionary.fill(u32::MAX);
        self.put(u32::from(last) | 2, 3); // BTYPE=01, least-significant bit first.
        let mut pos = 0;
        while pos + 3 <= data.len() {
            let (slot, prefix) = hash(data, pos);
            let entry = self.dictionary[slot];
            let stored = (entry & 65535) as usize;
            self.dictionary[slot] = (prefix << 16) | pos as u32;
            let maximum = (data.len() - pos).min(258);
            let repeated = if pos > 0 && data[pos] == data[pos - 1] {
                repeated_length(data, pos - 1, pos, maximum)
            } else {
                0
            };
            // The old tie-break already preferred distance one. A maximal run
            // cannot lose; retain the dictionary update for subsequent tokens.
            let (prior, length) = if repeated == 0 {
                (stored, match_candidate(data, entry, prefix, pos, maximum))
            } else if repeated == maximum {
                (pos - 1, repeated)
            } else {
                let matched = match_candidate(data, entry, prefix, pos, maximum);
                if repeated >= matched {
                    (pos - 1, repeated)
                } else {
                    (stored, matched)
                }
            };
            // One-step lazy parsing can emit a literal when the following
            // position offers a strictly longer match. RFC 1951 coding and
            // the 32 KiB history/distance limits are unchanged.
            if COMPACT && (3..258).contains(&length) && pos + 4 <= data.len() {
                let next_pos = pos + 1;
                let (next_slot, next_prefix) = hash(data, next_pos);
                let next_length = match_candidate(
                    data,
                    self.dictionary[next_slot],
                    next_prefix,
                    next_pos,
                    (data.len() - next_pos).min(258),
                );
                if next_length > length + 1 {
                    self.symbol(u32::from(data[pos]));
                    pos += 1;
                    continue;
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
        self.flush_bytes();
        // A stored block is preferable when fixed codes expand this input.
        // Restore the pending bits from the preceding block before retrying.
        if self.output.len() > data.len() + 5 {
            (self.bits, self.count) = saved;
            self.store(data, last);
        }
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build the documented dictionary entry independently of hash slot selection.
    fn match_length(data: &[u8], prior: usize, pos: usize, maximum: usize) -> usize {
        let prefix = u32::from(u16::from_le_bytes([data[pos], data[pos + 1]]));
        let stored = u32::from(u16::from_le_bytes([data[prior], data[prior + 1]]));
        match_candidate(data, (stored << 16) | prior as u32, prefix, pos, maximum)
    }

    #[test]
    fn dictionary_prefix_rejection_preserves_three_byte_matching() {
        let data = [1, 2, 3, 7, 9, 2, 3, 7, 1, 9, 3, 7, 1, 2, 9, 7, 1, 2, 3, 8];
        for (pos, length) in [(4, 0), (8, 0), (12, 0), (16, 3)] {
            assert_eq!(match_length(&data, 0, pos, 4), length);
        }
        assert_eq!(match_candidate(&[255; 3], u32::MAX, 65535, 0, 3), 0);
    }

    #[test]
    fn distance_one_scan_matches_scalar_at_every_termination() {
        for maximum in 3..=258 {
            for cut in 0..=maximum {
                let mut data = vec![19; maximum + 1];
                if cut < maximum {
                    data[cut + 1] = 37;
                }
                let expected = if cut < 3 { 0 } else { cut };
                assert_eq!(repeated_length(&data, 0, 1, maximum), expected);
            }
        }
    }

    #[test]
    fn long_symbol_sequence_matches_independent_bit_stream() {
        for pending in 0..8 {
            let mut encoder = Encoder::new(4096).unwrap();
            let mut expected = vec![true; pending];
            encoder.put((1 << pending) - 1, pending as u32);
            for symbol in 0..=285u32 {
                let (code, count) = match symbol {
                    0..=143 => (symbol + 48, 8),
                    144..=255 => (symbol - 144 + 400, 9),
                    256..=279 => (symbol - 256, 7),
                    _ => (symbol - 280 + 192, 8),
                };
                for bit in (0..count).rev() {
                    expected.push(code & (1 << bit) != 0);
                }
                encoder.symbol(symbol);
            }
            encoder.align();
            let bytes: Vec<u8> = expected
                .chunks(8)
                .map(|bits| {
                    bits.iter()
                        .enumerate()
                        .fold(0, |byte, (i, &bit)| byte | (u8::from(bit) << i))
                })
                .collect();
            assert_eq!(encoder.output, bytes);
            assert_eq!(encoder.count, 0);
        }
    }

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
                encoder.flush_bytes();
                assert_eq!(encoder.output, bits.to_le_bytes()[..bytes]);
                assert_eq!(encoder.bits, u64::from(bits >> (bytes * 8)));
                assert_eq!(encoder.count, total % 8);
            }
        }
    }

    #[test]
    fn matches_agree_with_scalar_scan_for_overlapping_slices() {
        for distance in [1, 2, 3, 4, 7, 8, 15, 16, 31, 32] {
            for maximum in 3..=258 {
                for cut in 3..=maximum {
                    let mut data = vec![19; distance + maximum];
                    if cut < maximum {
                        data[distance + cut] = 37;
                    }
                    let equal = data[..maximum]
                        .iter()
                        .zip(&data[distance..])
                        .take_while(|(a, b)| a == b)
                        .count();
                    let expected = if equal < 3 { 0 } else { equal };
                    assert_eq!(
                        match_length(&data, 0, distance, maximum),
                        expected,
                        "distance {distance}, maximum {maximum}, cut {cut}"
                    );
                }
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
                        actual.flush_bytes();
                        expected.flush_bytes();
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
    fn sampled_incompressibility_preserves_stored_bytes_and_compressible_tails() {
        let mut state = 0x6ad1_7e95u32;
        let mut noise = vec![0u8; BLOCK];
        for byte in &mut noise {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = (state >> 16) as u8;
        }
        let mut encoder = Encoder::new(BLOCK).unwrap();
        assert!(Encoder::probably_stored(&noise));
        let stored = encoder.block(&noise, true);
        assert_eq!(stored.len(), BLOCK + 5);
        assert_eq!(stored[0], 1);
        assert_eq!(&stored[5..], &noise);

        // No first-window-only shortcuts: a compressible tail must be seen.
        let mut tail = noise;
        tail[BLOCK - 4096..].fill(7);
        let mut encoder = Encoder::new(BLOCK).unwrap();
        assert!(!Encoder::probably_stored(&tail));
        assert!(encoder.block(&tail, true).len() < BLOCK + 5);

        // Every block must reset the sampled dictionary; preserved prior bits
        // allow a stored block after an earlier fixed block.
        let mut encoder = Encoder::new(BLOCK).unwrap();
        let first = encoder.block(&[0; 8192], false).to_vec();
        let last = encoder.block(&tail, true).to_vec();
        assert!(!first.is_empty() && !last.is_empty());
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
