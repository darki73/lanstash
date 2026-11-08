//! Checking a chunk read from elsewhere than the CDN (lancache's old cache):
//! the CRC-32 of its bytes must be the `x-content-crc` the CDN sent with it
//! (lancache keeps that header with the chunk's slices), and its length the
//! manifest's compressed size. A chunk
//! without the header fails, so it is fetched from the CDN instead.

use lanstash_domain::checksum::crc::Crc32;
use lanstash_domain::segment::meta::decode_meta;

/// The streaming check of one chunk.
#[derive(Debug, Clone)]
pub struct ChunkCheck {
    crc: Crc32,
    len: u64,
    want_len: u64,
    want_crc: Option<u32>,
}

impl ChunkCheck {
    /// A chunk of `size` bytes on the wire, with the response headers stored
    /// with it (lanstash's meta form).
    #[must_use]
    pub fn new(size: u64, stored: &[u8]) -> Self {
        let want_crc = decode_meta(stored)
            .find(|(n, _)| n.eq_ignore_ascii_case("x-content-crc"))
            .and_then(|(_, v)| v.trim().parse().ok());
        Self { crc: Crc32::new(), len: 0, want_len: size, want_crc }
    }

    /// The next bytes, in order.
    pub fn feed(&mut self, data: &[u8]) {
        self.crc.update(data);
        self.len += data.len() as u64;
    }

    /// Whether all of it was the chunk.
    #[must_use]
    pub fn finish(&self) -> bool {
        self.len == self.want_len && self.want_crc == Some(self.crc.finish())
    }
}
