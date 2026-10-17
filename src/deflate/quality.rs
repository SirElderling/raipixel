//! Experimental opt-in DEFLATE depth-64 lazy parser with guarded dynamic Huffman.
//! This code is isolated from the accepted fast/compact encoder to preserve
//! existing PNG output bytes. See benchmarks/png/experiment-register.md (C24).
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
    history: Vec<u8>,
    trial_joined: Vec<u8>,
    trial_heads: Vec<u32>,
    trial_links: Vec<u32>,
    trial_tokens: Vec<u16>,
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
        let mut history = Vec::new();
        history.try_reserve_exact(32_768).map_err(|_| Error::Allocation)?;
        let joined_capacity = max_block.checked_add(32_768)
            .ok_or(Error::Allocation)?;
        let mut trial_joined = Vec::new();
        trial_joined.try_reserve_exact(joined_capacity)
            .map_err(|_| Error::Allocation)?;
        let mut trial_heads = Vec::new();
        trial_heads.try_reserve_exact(HASH_SIZE)
            .map_err(|_| Error::Allocation)?;
        let mut trial_links = Vec::new();
        trial_links.try_reserve_exact(joined_capacity)
            .map_err(|_| Error::Allocation)?;
        let mut trial_tokens = Vec::new();
        trial_tokens.try_reserve_exact(max_block)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            dictionary,
            history,
            trial_joined,
            trial_heads,
            trial_links,
            trial_tokens,
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


impl Encoder {
    // Scratch only: retain 32 KiB of uncompressed input history across
    // consecutive DEFLATE blocks; encode the current block only.
    pub(crate) fn block_history(&mut self, data: &[u8], last: bool) -> Result<&[u8], Error> {
        const DEPTH: usize = 64;
        debug_assert!(data.len() <= BLOCK);
        let mut joined = std::mem::take(&mut self.trial_joined);
        joined.clear();
        assert!(joined.capacity() >= self.history.len() + data.len());
        joined.extend_from_slice(&self.history);
        joined.extend_from_slice(data);
        let start = self.history.len();
        let mut heads = std::mem::take(&mut self.trial_heads);
        heads.clear();
        assert!(heads.capacity() >= HASH_SIZE);
        heads.resize(HASH_SIZE, u32::MAX);
        let mut links = std::mem::take(&mut self.trial_links);
        links.clear();
        assert!(links.capacity() >= joined.len());
        links.resize(joined.len(), u32::MAX);
        for pos in 0..start {
            if pos + 3 > joined.len() { break; }
            let (slot, _) = hash(&joined, pos);
            links[pos] = heads[slot];
            heads[slot] = pos as u32;
        }
        let saved = (self.bits, self.count);
        let mut tokens = std::mem::take(&mut self.trial_tokens);
        tokens.clear();
        assert!(tokens.capacity() >= data.len());
        self.output.clear();
        let mut pos = start;
        while pos + 3 <= joined.len() {
            let (slot, _) = hash(&joined, pos);
            let maximum = (joined.len() - pos).min(258);
            let mut best = 0usize;
            let mut best_pos = 0usize;
            let mut prior = heads[slot];
            for _ in 0..DEPTH {
                if prior == u32::MAX { break; }
                let candidate = prior as usize;
                if pos - candidate > 32_768 { break; }
                if joined[candidate..candidate + 3] == joined[pos..pos + 3] {
                    let length = 3 + matching_tail(
                        &joined[candidate + 3..candidate + maximum],
                        &joined[pos + 3..pos + maximum],
                    );
                    if length > best {
                        best = length;
                        best_pos = candidate;
                        if length == maximum { break; }
                    }
                }
                prior = links[candidate];
            }
            links[pos] = heads[slot];
            heads[slot] = pos as u32;
            if pos > 0 && joined[pos] == joined[pos - 1] {
                let length = repeated_length(&joined, pos - 1, pos, maximum);
                if length >= best {
                    best = length;
                    best_pos = pos - 1;
                }
            }
            if best >= 3 && best < 258 && pos + 4 <= joined.len() {
                let next = pos + 1;
                let (next_slot, _) = hash(&joined, next);
                let mut node = heads[next_slot];
                let mut next_best = 0usize;
                let max_next = (joined.len() - next).min(258);
                for _ in 0..DEPTH {
                    if node == u32::MAX { break; }
                    let candidate = node as usize;
                    if next - candidate > 32_768 { break; }
                    if joined[candidate..candidate+3] == joined[next..next+3] {
                        let length = 3 + matching_tail(
                            &joined[candidate+3..candidate+max_next],
                            &joined[next+3..next+max_next],
                        );
                        next_best = next_best.max(length);
                    }
                    node = links[candidate];
                }
                // One extra literal may permit a longer next match.
                if next_best > best + 1 {
                    tokens.push(u16::from(joined[pos]));
                    pos += 1;
                    continue;
                }
            }
            if best >= 3 {
                tokens.push(256 + (best - 3) as u16);
                tokens.push((pos - best_pos) as u16);
                let end = pos + best;
                pos += 1;
                while pos < end {
                    if pos + 3 <= joined.len() {
                        let (slot, _) = hash(&joined, pos);
                        links[pos] = heads[slot];
                        heads[slot] = pos as u32;
                    }
                    pos += 1;
                }
            } else {
                tokens.push(u16::from(joined[pos]));
                pos += 1;
            }
        }
        for &byte in &joined[pos..] { tokens.push(u16::from(byte)); }
        let fixed_bits = trial_fixed_bits(&tokens);
        let fixed_bytes = ((saved.1 + fixed_bits + if last { 7 } else { 0 }) / 8) as usize;
        // Skip all tree work for tiny outputs and fall back if no savings.
        let dynamic_ok = data.len() >= 4096 && self.trial_emit_dynamic(&tokens, last);
        if !dynamic_ok || self.output.len() >= fixed_bytes.min(data.len() + 5) {
            self.output.clear();
            (self.bits, self.count) = saved;
            if fixed_bytes > data.len() + 5 {
                self.store(data, last);
            } else {
                self.put(u32::from(last) | 2, 3);
                for token in trial_unpack(&tokens) {
                    match token {
                        TrialToken::Literal(value) => self.symbol(u32::from(value)),
                        TrialToken::Reference(length, distance) => self.reference(length, distance),
                    }
                }
                self.symbol(256);
                if last { self.align(); }
                self.flush_bytes();
            }
        }
        // The decompressor sees stored and Huffman-coded bytes equally:
        // either form contributes to the same sliding history.
        let keep = joined.len().min(32_768);
        self.history.clear();
        self.history.extend_from_slice(&joined[joined.len() - keep..]);
        self.trial_joined = joined;
        self.trial_heads = heads;
        self.trial_links = links;
        self.trial_tokens = tokens;
        Ok(&self.output)
    }
}

#[derive(Clone,Copy)]
enum TrialToken { Literal(u8), Reference(usize,usize) }

// Bounded token storage: a literal needs one u16, a match two u16.
// Each match consumes at least three input bytes, so at most 65,535
// words (128 KiB) per PNG block.
fn trial_unpack(tokens:&[u16])->impl Iterator<Item=TrialToken>+'_{
    struct Tokens<'a>{input:&'a[u16],index:usize}
    impl Iterator for Tokens<'_>{
        type Item=TrialToken;
        fn next(&mut self)->Option<TrialToken>{
            let &word=self.input.get(self.index)?;
            self.index+=1;
            if word<256{
                Some(TrialToken::Literal(word as u8))
            }else{
                let &distance=self.input.get(self.index)?;
                self.index+=1;
                Some(TrialToken::Reference((word-256) as usize+3,distance as usize))
            }
        }
    }
    Tokens{input:tokens,index:0}
}

#[derive(Clone)]
struct TrialNode { freq:u32, left:Option<usize>, right:Option<usize>, sym:Option<usize> }

// Scratch-only bounded-size Huffman trees. If an optimal tree exceeds DEFLATE
// length caps, reject the candidate and use accepted fixed/stored output.
fn trial_lengths(freq:&[u32],max_depth:u8)->Option<Vec<u8>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut nodes=Vec::<TrialNode>::new();
    nodes.try_reserve_exact(freq.len().saturating_mul(2)).ok()?;
    let mut queue=BinaryHeap::<Reverse<(u32,usize)>>::new();
    queue.try_reserve(freq.len()).ok()?;
    for (symbol,&weight) in freq.iter().enumerate(){
        if weight!=0{
            let index=nodes.len();
            nodes.push(TrialNode{freq:weight,left:None,right:None,sym:Some(symbol)});
            queue.push(Reverse((weight,index)));
        }
    }
    if queue.is_empty(){return None;}
    let mut lengths=Vec::new();
    lengths.try_reserve_exact(freq.len()).ok()?;
    lengths.resize(freq.len(),0u8);
    if queue.len()==1 {
        let Reverse((_,index))=queue.pop()?;
        lengths[nodes[index].sym?]=1;
        return Some(lengths);
    }
    while queue.len()>1 {
        let Reverse((left_weight,left))=queue.pop()?;
        let Reverse((right_weight,right))=queue.pop()?;
        let weight=left_weight.checked_add(right_weight)?;
        let index=nodes.len();
        nodes.push(TrialNode{freq:weight,left:Some(left),right:Some(right),sym:None});
        queue.push(Reverse((weight,index)));
    }
    let Reverse((_,root))=queue.pop()?;
    let mut stack=Vec::new();
    stack.try_reserve_exact(nodes.len()).ok()?;
    stack.push((root,0u8));
    while let Some((node,depth))=stack.pop(){
        if depth>max_depth{return None;}
        if let Some(symbol)=nodes[node].sym{
            lengths[symbol]=depth;
        }else{
            stack.push((nodes[node].left?,depth+1));
            stack.push((nodes[node].right?,depth+1));
        }
    }
    Some(lengths)
}
fn trial_codes(lengths:&[u8])->Option<Vec<(u16,u8)>> {
    let mut count=[0u16;16];
    for &length in lengths {
        if length!=0 {count[usize::from(length)]+=1;}
    }
    let mut next=[0u16;16];
    for n in 1..16{next[n]=(next[n-1]+count[n-1])<<1;}
    let mut result=Vec::new();
    result.try_reserve_exact(lengths.len()).ok()?;
    result.resize(lengths.len(),(0u16,0u8));
    for (symbol,&length) in lengths.iter().enumerate(){
        if length!=0 {
            let code=next[usize::from(length)];
            next[usize::from(length)]+=1;
            result[symbol]=(code.reverse_bits()>>(16-length),length);
        }
    }
    Some(result)
}
fn trial_rle(lengths:&[u8])->Option<Vec<(usize,u32,u32)>> {
    let mut output=Vec::new();
    output.try_reserve_exact(lengths.len()).ok()?;
    let mut i=0;
    while i<lengths.len(){
        let value=lengths[i];
        let mut end=i+1;
        while end<lengths.len() && lengths[end]==value {end+=1;}
        let mut n=end-i;
        if value==0 {
            while n>=11 {
                let take=n.min(138);
                output.push((18,(take-11) as u32,7));
                n-=take;
            }
            while n>=3 {
                let take=n.min(10);
                output.push((17,(take-3) as u32,3));
                n-=take;
            }
            for _ in 0..n {output.push((0,0,0));}
        }else{
            output.push((usize::from(value),0,0));
            n-=1;
            while n>=3 {
                let take=n.min(6);
                output.push((16,(take-3) as u32,2));
                n-=take;
            }
            for _ in 0..n {output.push((usize::from(value),0,0));}
        }
        i=end;
    }
    Some(output)
}
// Fixed-code bit cost: three-bit header, seven-bit end marker and tokens.
fn trial_fixed_bits(tokens: &[u16]) -> u32 {
    let mut bits = 3 + 7;
    for token in trial_unpack(tokens) {
        match token {
            TrialToken::Literal(value) => {
                bits += u32::from(FIXED_CODES[usize::from(value)] >> 9);
            }
            TrialToken::Reference(length, distance) => {
                let index = if length == 258 { 28 } else { usize::from(LENGTH_CODES[length-3]) };
                bits += u32::from(FIXED_CODES[257+index] >> 9) + LENGTH_EXTRA[index];
                let dindex = distance_code(distance);
                bits += 5 + DISTANCE_EXTRA[dindex];
            }
        }
    }
    bits
}

impl Encoder {
    fn trial_put_code(&mut self,codes:&[(u16,u8)],symbol:usize){
        let (code,n)=codes[symbol];
        self.put(u32::from(code),u32::from(n));
    }
    fn trial_emit_dynamic(&mut self,tokens:&[u16],last:bool)->bool{
        const ORDER:[usize;19]=[16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
        let mut lf=[0u32;286];
        let mut df=[0u32;30];
        for token in trial_unpack(tokens) {
            match token {
                TrialToken::Literal(v)=>lf[usize::from(v)]+=1,
                TrialToken::Reference(len,dist)=>{
                    let l=if len==258{285}else{257+usize::from(LENGTH_CODES[len-3])};
                    lf[l]+=1;
                    df[distance_code(dist)]+=1;
                }
            }
        }
        lf[256]+=1;
        if df.iter().all(|&x|x==0){df[0]=1;}
        let Some(lens)=trial_lengths(&lf,15) else{return false;};
        let Some(dist_lens)=trial_lengths(&df,15) else{return false;};
        let ll_count=lens.iter().rposition(|&v|v!=0).unwrap_or(256).max(256)+1;
        let ds_count=dist_lens.iter().rposition(|&v|v!=0).unwrap_or(0)+1;
        let mut together=Vec::new();
        if together.try_reserve_exact(ll_count+ds_count).is_err() {return false;}
        together.extend_from_slice(&lens[..ll_count]);
        together.extend_from_slice(&dist_lens[..ds_count]);
        let Some(runs)=trial_rle(&together) else{return false;};
        let mut cf=[0u32;19];
        for &(sym,_,_) in &runs{cf[sym]+=1;}
        if cf.iter().filter(|&&v|v!=0).count()==1{
            if let Some(slot)=cf.iter().position(|&v|v==0){cf[slot]=1;}
        }
        let Some(code_lens)=trial_lengths(&cf,7) else{return false;};
        let cl_count=ORDER.iter().rposition(|&s|code_lens[s]!=0).unwrap_or(3).max(3)+1;
        let Some(lc)=trial_codes(&lens) else{return false;};
        let Some(dc)=trial_codes(&dist_lens) else{return false;};
        let Some(cc)=trial_codes(&code_lens) else{return false;};
        self.put(u32::from(last)|4,3); // BTYPE=10, dynamic.
        self.put((ll_count-257) as u32,5);
        self.put((ds_count-1) as u32,5);
        self.put((cl_count-4) as u32,4);
        for &sym in &ORDER[..cl_count]{self.put(u32::from(code_lens[sym]),3);}
        for &(sym,extra,count) in &runs{
            self.trial_put_code(&cc,sym);
            self.put(extra,count);
        }
        for token in trial_unpack(tokens) {
            match token {
                TrialToken::Literal(v)=>self.trial_put_code(&lc,usize::from(v)),
                TrialToken::Reference(len,dist)=>{
                    let index=if len==258{28}else{usize::from(LENGTH_CODES[len-3])};
                    self.trial_put_code(&lc,257+index);
                    self.put((len-LENGTH_BASE[index]) as u32,LENGTH_EXTRA[index]);
                    let dindex=distance_code(dist);
                    self.trial_put_code(&dc,dindex);
                    self.put((dist-DISTANCE_BASE[dindex]) as u32,DISTANCE_EXTRA[dindex]);
                }
            }
        }
        self.trial_put_code(&lc,256);
        if last{self.align();}
        self.flush_bytes();
        true
    }
}