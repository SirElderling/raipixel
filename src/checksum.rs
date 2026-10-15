// Independently implemented from the CRC32 and Adler32 definitions in PNG/RFC 1950.
const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut value = i as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 1 {
                (value >> 1) ^ 0xedb8_8320
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[i] = value;
        i += 1;
    }
    table
}
const CRC_TABLE: [u32; 256] = crc_table();

pub(crate) fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for part in parts {
        for &byte in *part {
            crc = (crc >> 8) ^ CRC_TABLE[((crc ^ u32::from(byte)) & 255) as usize];
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
        // u64 bounds permit 65,535 bytes between reductions, including the
        // largest stored block, without the smaller u32 accumulation bound.
        for block in data.chunks(65_535) {
            for &byte in block {
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
