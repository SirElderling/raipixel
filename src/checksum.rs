// Slicing-by-eight technique: Joglekar, Kounavis & Berry, FAST 2005.
// Independent implementation; attribution/context: benchmarks/png/algorithm-notes.md.
// Independently implemented from PNG/RFC 1950 polynomial/checksum definitions.
// Each additional CRC table represents one more trailing zero byte. Combining
// eight independent byte contributions removes the serial per-byte recurrence.
const fn crc_tables() -> [[u32; 256]; 8] {
    let mut tables = [[0; 256]; 8];
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
    while level < 8 {
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
static CRC: [[u32; 256]; 8] = crc_tables();

pub(crate) fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for part in parts {
        let (bulk, tail) = part.as_chunks::<8>();
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
            let (bulk, tail) = block.as_chunks::<8>();
            for bytes in bulk {
                let [x0, x1, x2, x3, x4, x5, x6, x7] = bytes.map(u64::from);
                self.b +=
                    8 * self.a + 8 * x0 + 7 * x1 + 6 * x2 + 5 * x3 + 4 * x4 + 3 * x5 + 2 * x6 + x7;
                self.a += x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7;
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
