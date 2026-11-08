//! Reflected CRC-32 variants, table-driven "slicing-by-8", safe Rust.
//!
//! - [`Crc32c`] (Castagnoli, `0x82F63B78`): segment, record and footer checksums.
//! - [`Crc32`] (IEEE / zlib, `0xEDB88320`): Steam's `x-content-crc` header, which
//!   is the CRC-32 of the whole chunk as served.

const fn build_tables(poly: u32) -> [[u32; 256]; 8] {
    let mut t = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ poly } else { c >> 1 };
            k += 1;
        }
        t[0][i] = c;
        i += 1;
    }
    let mut s = 1;
    while s < 8 {
        let mut i = 0;
        while i < 256 {
            let prev = t[s - 1][i];
            t[s][i] = (prev >> 8) ^ t[0][(prev & 0xFF) as usize];
            i += 1;
        }
        s += 1;
    }
    t
}

/// Incremental reflected CRC-32 with polynomial `POLY` (init and xor-out `!0`).
#[derive(Clone, Copy, Debug)]
pub struct Crc<const POLY: u32> {
    state: u32,
}

/// CRC-32C (Castagnoli).
pub type Crc32c = Crc<0x82F6_3B78>;
/// CRC-32 (IEEE 802.3 / zlib / gzip).
pub type Crc32 = Crc<0xEDB8_8320>;

impl<const POLY: u32> Default for Crc<POLY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const POLY: u32> Crc<POLY> {
    const TABLES: [[u32; 256]; 8] = build_tables(POLY);

    #[must_use]
    pub const fn new() -> Self {
        Self { state: !0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        let t = &Self::TABLES;
        let mut crc = self.state;
        let mut chunks = data.chunks_exact(8);
        for c in &mut chunks {
            let lo = u32::from_le_bytes([c[0], c[1], c[2], c[3]]) ^ crc;
            let hi = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
            crc = t[7][(lo & 0xFF) as usize]
                ^ t[6][((lo >> 8) & 0xFF) as usize]
                ^ t[5][((lo >> 16) & 0xFF) as usize]
                ^ t[4][(lo >> 24) as usize]
                ^ t[3][(hi & 0xFF) as usize]
                ^ t[2][((hi >> 8) & 0xFF) as usize]
                ^ t[1][((hi >> 16) & 0xFF) as usize]
                ^ t[0][(hi >> 24) as usize];
        }
        for &b in chunks.remainder() {
            crc = (crc >> 8) ^ t[0][((crc ^ u32::from(b)) & 0xFF) as usize];
        }
        self.state = crc;
    }

    #[must_use]
    pub const fn finish(&self) -> u32 {
        !self.state
    }

    /// One-shot checksum of `data`.
    #[must_use]
    pub fn checksum(data: &[u8]) -> u32 {
        let mut c = Self::new();
        c.update(data);
        c.finish()
    }
}

/// One-shot CRC-32C of `data`.
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    Crc32c::checksum(data)
}

/// One-shot CRC-32 (IEEE) of `data`.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    Crc32::checksum(data)
}
