// Slicing technique: Joglekar, Kounavis & Berry, FAST 2005.
// Independent implementation; attribution/context: benchmarks/png/algorithm-notes.md.
// Independently implemented from PNG/RFC 1950 polynomial/checksum definitions.
// Each additional CRC table represents one more trailing zero byte. Combining
// independent byte contributions reduces the serial per-byte recurrence.
// Native CRC benchmarks favor 32-byte slices on x86-64 and 64-byte
// slices on AArch64. Other CPUs use the portable eight-byte path.
#[cfg(target_arch = "aarch64")]
const CRC_TABLE_COUNT: usize = 64;
#[cfg(target_arch = "x86_64")]
const CRC_TABLE_COUNT: usize = 32;
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const CRC_TABLE_COUNT: usize = 8;

const fn crc_tables() -> [[u32; 256]; CRC_TABLE_COUNT] {
    let mut tables = [[0; 256]; CRC_TABLE_COUNT];
    let mut i = 0;
    while i < 256 {
        let mut value = i as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ if value & 1 != 0 { 0xedb8_8320 } else { 0 };
            bit += 1;
        }
        tables[0][i] = value;
        i += 1;
    }
    let mut level = 1;
    while level < CRC_TABLE_COUNT {
        let mut i = 0;
        while i < 256 {
            let prior = tables[level - 1][i];
            tables[level][i] = (prior >> 8) ^ tables[0][(prior & 255) as usize];
            i += 1;
        }
        level += 1;
    }
    tables
}
static CRC: [[u32; 256]; CRC_TABLE_COUNT] = crc_tables();

pub(crate) fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for part in parts {
        // 32-byte slicing for large x86-64 chunks, preserving the existing
        // eight-byte remainder on short inputs.
        #[cfg(target_arch = "x86_64")]
        let (wide, tail) = if part.len() >= 4096 {
            part.as_chunks::<32>()
        } else {
            (&[][..], *part)
        };
        #[cfg(target_arch = "x86_64")]
        for bytes in wide {
            let head = u32::from_le_bytes(bytes[..4].try_into().unwrap()) ^ crc;
            crc = CRC[31][(head & 255) as usize]
                ^ CRC[30][((head >> 8) & 255) as usize]
                ^ CRC[29][((head >> 16) & 255) as usize]
                ^ CRC[28][((head >> 24) & 255) as usize]
                ^ CRC[27][bytes[4] as usize]
                ^ CRC[26][bytes[5] as usize]
                ^ CRC[25][bytes[6] as usize]
                ^ CRC[24][bytes[7] as usize]
                ^ CRC[23][bytes[8] as usize]
                ^ CRC[22][bytes[9] as usize]
                ^ CRC[21][bytes[10] as usize]
                ^ CRC[20][bytes[11] as usize]
                ^ CRC[19][bytes[12] as usize]
                ^ CRC[18][bytes[13] as usize]
                ^ CRC[17][bytes[14] as usize]
                ^ CRC[16][bytes[15] as usize]
                ^ CRC[15][bytes[16] as usize]
                ^ CRC[14][bytes[17] as usize]
                ^ CRC[13][bytes[18] as usize]
                ^ CRC[12][bytes[19] as usize]
                ^ CRC[11][bytes[20] as usize]
                ^ CRC[10][bytes[21] as usize]
                ^ CRC[9][bytes[22] as usize]
                ^ CRC[8][bytes[23] as usize]
                ^ CRC[7][bytes[24] as usize]
                ^ CRC[6][bytes[25] as usize]
                ^ CRC[5][bytes[26] as usize]
                ^ CRC[4][bytes[27] as usize]
                ^ CRC[3][bytes[28] as usize]
                ^ CRC[2][bytes[29] as usize]
                ^ CRC[1][bytes[30] as usize]
                ^ CRC[0][bytes[31] as usize];
        }
        // 64-byte slicing on AArch64 lowers the dependent checksum chain
        // without intrinsics, target features, or unsafe code.
        #[cfg(target_arch = "aarch64")]
        let (wide, tail) = if part.len() >= 4096 {
            part.as_chunks::<64>()
        } else {
            (&[][..], *part)
        };
        #[cfg(target_arch = "aarch64")]
        for bytes in wide {
            let head = u32::from_le_bytes(bytes[..4].try_into().unwrap()) ^ crc;
            crc = CRC[63][(head & 255) as usize]
                ^ CRC[62][((head >> 8) & 255) as usize]
                ^ CRC[61][((head >> 16) & 255) as usize]
                ^ CRC[60][((head >> 24) & 255) as usize]
                ^ CRC[59][bytes[4] as usize]
                ^ CRC[58][bytes[5] as usize]
                ^ CRC[57][bytes[6] as usize]
                ^ CRC[56][bytes[7] as usize]
                ^ CRC[55][bytes[8] as usize]
                ^ CRC[54][bytes[9] as usize]
                ^ CRC[53][bytes[10] as usize]
                ^ CRC[52][bytes[11] as usize]
                ^ CRC[51][bytes[12] as usize]
                ^ CRC[50][bytes[13] as usize]
                ^ CRC[49][bytes[14] as usize]
                ^ CRC[48][bytes[15] as usize]
                ^ CRC[47][bytes[16] as usize]
                ^ CRC[46][bytes[17] as usize]
                ^ CRC[45][bytes[18] as usize]
                ^ CRC[44][bytes[19] as usize]
                ^ CRC[43][bytes[20] as usize]
                ^ CRC[42][bytes[21] as usize]
                ^ CRC[41][bytes[22] as usize]
                ^ CRC[40][bytes[23] as usize]
                ^ CRC[39][bytes[24] as usize]
                ^ CRC[38][bytes[25] as usize]
                ^ CRC[37][bytes[26] as usize]
                ^ CRC[36][bytes[27] as usize]
                ^ CRC[35][bytes[28] as usize]
                ^ CRC[34][bytes[29] as usize]
                ^ CRC[33][bytes[30] as usize]
                ^ CRC[32][bytes[31] as usize]
                ^ CRC[31][bytes[32] as usize]
                ^ CRC[30][bytes[33] as usize]
                ^ CRC[29][bytes[34] as usize]
                ^ CRC[28][bytes[35] as usize]
                ^ CRC[27][bytes[36] as usize]
                ^ CRC[26][bytes[37] as usize]
                ^ CRC[25][bytes[38] as usize]
                ^ CRC[24][bytes[39] as usize]
                ^ CRC[23][bytes[40] as usize]
                ^ CRC[22][bytes[41] as usize]
                ^ CRC[21][bytes[42] as usize]
                ^ CRC[20][bytes[43] as usize]
                ^ CRC[19][bytes[44] as usize]
                ^ CRC[18][bytes[45] as usize]
                ^ CRC[17][bytes[46] as usize]
                ^ CRC[16][bytes[47] as usize]
                ^ CRC[15][bytes[48] as usize]
                ^ CRC[14][bytes[49] as usize]
                ^ CRC[13][bytes[50] as usize]
                ^ CRC[12][bytes[51] as usize]
                ^ CRC[11][bytes[52] as usize]
                ^ CRC[10][bytes[53] as usize]
                ^ CRC[9][bytes[54] as usize]
                ^ CRC[8][bytes[55] as usize]
                ^ CRC[7][bytes[56] as usize]
                ^ CRC[6][bytes[57] as usize]
                ^ CRC[5][bytes[58] as usize]
                ^ CRC[4][bytes[59] as usize]
                ^ CRC[3][bytes[60] as usize]
                ^ CRC[2][bytes[61] as usize]
                ^ CRC[1][bytes[62] as usize]
                ^ CRC[0][bytes[63] as usize];
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        let tail = *part;
        let (bulk, tail) = tail.as_chunks::<8>();
        for bytes in bulk {
            let value = u64::from_le_bytes(*bytes) ^ u64::from(crc);
            crc = CRC[7][(value & 255) as usize]
                ^ CRC[6][((value >> 8) & 255) as usize]
                ^ CRC[5][((value >> 16) & 255) as usize]
                ^ CRC[4][((value >> 24) & 255) as usize]
                ^ CRC[3][((value >> 32) & 255) as usize]
                ^ CRC[2][((value >> 40) & 255) as usize]
                ^ CRC[1][((value >> 48) & 255) as usize]
                ^ CRC[0][(value >> 56) as usize];
        }
        for &byte in tail {
            crc = (crc >> 8) ^ CRC[0][((crc ^ u32::from(byte)) & 255) as usize];
        }
    }
    !crc
}

pub(crate) struct Adler {
    a: u64,
    b: u64,
}
impl Adler {
    pub(crate) fn new() -> Self {
        Self { a: 1, b: 0 }
    }
    pub(crate) fn update(&mut self, data: &[u8]) {
        // u64 accumulation permits whole stored blocks between reductions.
        for block in data.chunks(65_535) {
            let (bulk, tail) = block.as_chunks::<128>();
            // Two interleaved streams each accumulate eight eight-byte words.
            // Packed 16-bit sum/prior fields cannot carry (<= 2040/7140).
            // A byte at 16*k + 8*stream + j has weight
            // 16*(7-k) + 8*(1-stream) + (8-j) in B.
            // Derivation and Adler/RFC 1950 credit: algorithm-notes.md.
            for group in bulk {
                let (mut even, mut odd, mut prior_even, mut prior_odd) =
                    ([0u64; 2], [0u64; 2], [0u64; 2], [0u64; 2]);
                for bytes in group.as_chunks::<16>().0 {
                    for (stream, word) in bytes.as_chunks::<8>().0.iter().enumerate() {
                        let word = u64::from_le_bytes(*word);
                        prior_even[stream] += even[stream];
                        prior_odd[stream] += odd[stream];
                        even[stream] += word & 0x00ff_00ff_00ff_00ff;
                        odd[stream] += (word >> 8) & 0x00ff_00ff_00ff_00ff;
                    }
                }
                self.b += 128 * self.a;
                for stream in 0..2 {
                    let sums = even[stream] + odd[stream];
                    // Four sum fields total <= 16320, so no prefix carries.
                    let sum_total = sums.wrapping_mul(0x0001_0001_0001_0001) >> 48;
                    let odd_total = odd[stream].wrapping_mul(0x0001_0001_0001_0001) >> 48;
                    let prior = prior_even[stream] + prior_odd[stream];
                    let pairs =
                        (prior & 0x0000_ffff_0000_ffff) + ((prior >> 16) & 0x0000_ffff_0000_ffff);
                    let prior_total = (pairs & 0xffff_ffff) + (pairs >> 32);
                    let weighted = 4 * (sums & 65535)
                        + 3 * ((sums >> 16) & 65535)
                        + 2 * ((sums >> 32) & 65535)
                        + (sums >> 48);
                    self.b += 16 * prior_total + 8 * (1 - stream) as u64 * sum_total + 2 * weighted
                        - odd_total;
                    self.a += sum_total;
                }
            }
            for &byte in tail {
                self.a += u64::from(byte);
                self.b += self.a;
            }
            self.a %= 65_521;
            self.b %= 65_521;
        }
    }
    pub(crate) fn value(&self) -> u32 {
        ((self.b << 16) | self.a) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_matches_standard_vector_and_polynomial_for_bulk_and_fragmented_input() {
        assert_eq!(crc32(&[b"123456789"]), 0xcbf4_3926);
        let data: Vec<u8> = (0..1024).map(|i| (i * 37 + 11) as u8).collect();
        let mut expected = u32::MAX;
        for &byte in &data {
            expected ^= u32::from(byte);
            for _ in 0..8 {
                expected = (expected >> 1) ^ if expected & 1 != 0 { 0xedb8_8320 } else { 0 };
            }
        }
        assert_eq!(crc32(&[&data]), !expected);
        for split in 0..32 {
            assert_eq!(crc32(&[&data[..split], &data[split..]]), !expected);
        }
    }
    #[test]
    fn crc_matches_polynomial_across_bulk_boundaries_and_fragmentation() {
        let mut seed = 17u32;
        let data: Vec<u8> = (0..65_537)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as u8
            })
            .collect();
        for len in (0..=129).chain([255, 256, 257, 4095, 4096, 4097, 65_535, 65_536, 65_537]) {
            let mut expected = u32::MAX;
            for &byte in &data[..len] {
                expected ^= u32::from(byte);
                for _ in 0..8 {
                    expected = (expected >> 1) ^ if expected & 1 != 0 { 0xedb8_8320 } else { 0 };
                }
            }
            for split in 0..=len.min(65) {
                assert_eq!(
                    crc32(&[&data[..split], &[], &data[split..len]]),
                    !expected,
                    "length {len}, split {split}"
                );
            }
        }
    }

    #[test]
    fn crc_matches_bitwise_polynomial_at_architecture_and_threshold_boundaries() {
        let mut state = 0xb34e_6a91u32;
        let data: Vec<u8> = (0..65_537)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 16) as u8
            })
            .collect();
        for len in [
            0, 1, 7, 8, 31, 32, 33, 63, 64, 65, 4095, 4096, 4097,
            8191, 8192, 8193, 65_535, 65_536, 65_537,
        ] {
            let mut expected = u32::MAX;
            for &byte in &data[..len] {
                expected ^= u32::from(byte);
                for _ in 0..8 {
                    expected = (expected >> 1)
                        ^ if expected & 1 != 0 { 0xedb8_8320 } else { 0 };
                }
            }
            for split in [0, 1, 7, 8, 31, 32, 63, 64, len / 2, len] {
                if split <= len {
                    assert_eq!(
                        crc32(&[&data[..split], &[], &data[split..len]]),
                        !expected,
                        "length {len}, split {split}"
                    );
                }
            }
        }
    }

    #[test]
    fn adler_preserves_each_byte_weight_in_a_full_group() {
        // The defining recurrence gives byte i weight (length - i) in B.
        // A single nonzero sample isolates that coefficient independently.
        for position in 0..128 {
            let mut data = [0u8; 128];
            data[position] = 255;
            let mut adler = Adler::new();
            adler.update(&data);
            let expected_b = 128 + 255 * (128 - position as u32);
            assert_eq!(adler.value(), (expected_b << 16) | 256);
        }
    }
    #[test]
    fn adler_bulk_accumulators_handle_full_blocks() {
        let data = vec![255; 2 * 65_535 + 17];
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in &data {
            a = (a + u32::from(byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        for chunk_size in [15, 16, 17, 127, 128, 129, 65_535, data.len()] {
            let mut adler = Adler::new();
            for part in data.chunks(chunk_size) {
                adler.update(part);
            }
            assert_eq!(adler.value(), (b << 16) | a);
        }
    }
    #[test]
    fn adler_matches_standard_vector_and_incremental_updates() {
        let mut adler = Adler::new();
        adler.update(b"123456789");
        assert_eq!(adler.value(), 0x091e_01de);
        let data: Vec<u8> = (0..70_000).map(|i| (i * 17 + 3) as u8).collect();
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in &data {
            a = (a + u32::from(byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        let expected = (b << 16) | a;
        let mut whole = Adler::new();
        whole.update(&data);
        assert_eq!(whole.value(), expected);
        let mut fragmented = Adler::new();
        for part in data.chunks(13) {
            fragmented.update(part);
        }
        assert_eq!(fragmented.value(), expected);
    }
}
