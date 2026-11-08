//! BLTE, the container Battle.net's CDN keeps files in: a header listing
//! chunks, each chunk a mode byte and its data. Modes `N` (stored) and `Z`
//! (zlib, [`crate::inflate`]) are decoded; `E` (encrypted) and `F` (nested)
//! are refused: the download manifests read here use neither.
//!
//! Layout: `"BLTE"`, header size (u32 BE; 0 = one chunk, the rest of the
//! file), then, when not 0, flags (u8, `0x0F`), chunk count (u24 BE) and per
//! chunk compressed size, decompressed size (u32 BE each) and an MD5 of the
//! compressed chunk (checked by [`verify`], not by [`decode`]).

use alloc::vec::Vec;

use lanstash_domain::checksum::md5::{Md5, md5};
use lanstash_domain::inflate::{InflateError, zlib};

/// Why a BLTE file could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlteError {
    /// Not BLTE, or a header that does not add up.
    BadHeader,
    /// The file ends inside a chunk.
    Truncated,
    /// A chunk in a mode not decoded here (`E`, `F`, ...): the mode byte.
    Unsupported(u8),
    /// A chunk's decoded size differs from the header's.
    BadSize,
    /// A `Z` chunk that did not inflate.
    Inflate(InflateError),
    /// The output would exceed the caller's limit.
    TooLarge,
}

fn be(b: &[u8]) -> u32 {
    b.iter().fold(0, |a, &x| (a << 8) | u32::from(x))
}

/// Decodes a BLTE file, producing at most `max` bytes.
///
/// # Errors
/// [`BlteError`].
pub fn decode(data: &[u8], max: usize) -> Result<Vec<u8>, BlteError> {
    if data.len() < 8 || &data[..4] != b"BLTE" {
        return Err(BlteError::BadHeader);
    }
    let header = be(&data[4..8]) as usize;
    // (compressed size, decompressed size if known)
    let mut chunks: Vec<(usize, Option<usize>)> = Vec::new();
    let mut pos = 8;
    if header == 0 {
        chunks.push((data.len() - 8, None));
    } else {
        let h = data.get(..header).ok_or(BlteError::Truncated)?;
        if h.len() < 12 || h[8] != 0x0F {
            return Err(BlteError::BadHeader);
        }
        let n = be(&h[9..12]) as usize;
        if n == 0 || 12 + n * 24 != header {
            return Err(BlteError::BadHeader);
        }
        for c in h[12..].chunks_exact(24) {
            chunks.push((be(&c[..4]) as usize, Some(be(&c[4..8]) as usize)));
        }
        pos = header;
    }
    let mut out = Vec::new();
    for (comp, decomp) in chunks {
        let c = data.get(pos..pos + comp).ok_or(BlteError::Truncated)?;
        pos += comp;
        let (&mode, body) = c.split_first().ok_or(BlteError::BadHeader)?;
        let room = max - out.len();
        let before = out.len();
        match mode {
            b'N' => {
                if body.len() > room {
                    return Err(BlteError::TooLarge);
                }
                out.extend_from_slice(body);
            }
            b'Z' => out.extend(zlib(body, room).map_err(|e| match e {
                InflateError::TooLarge => BlteError::TooLarge,
                e => BlteError::Inflate(e),
            })?),
            m => return Err(BlteError::Unsupported(m)),
        }
        if decomp.is_some_and(|d| d != out.len() - before) {
            return Err(BlteError::BadSize);
        }
    }
    Ok(out)
}

/// Whether `data` is exactly the BLTE file whose encoding key is `ekey`,
/// without decoding it: the key is the MD5 of the header (of the whole file
/// when the header size is 0), and every chunk matches the MD5 the header
/// lists for it, the chunks filling the file exactly (the header-size-0 case
/// is a **hypothesis**). [`Verifier`] does the same over parts.
#[must_use]
pub fn verify(data: &[u8], ekey: &[u8; 16]) -> bool {
    let mut v = Verifier::new(*ekey);
    v.feed(data);
    v.finish()
}

/// The longest header a [`Verifier`] collects: a table of 2^20 chunks (a file of a
/// gigabyte or more in chunks of a kilobyte). A longer one fails at once.
pub const MAX_HEADER: usize = 12 + 24 * (1 << 20);

/// Checks a BLTE file against its encoding key as its bytes arrive, keeping
/// no more than its header: as [`verify`].
#[derive(Debug, Clone)]
pub struct Verifier {
    ekey: [u8; 16],
    /// The header so far (until its size is known and it is whole).
    head: Vec<u8>,
    /// The header's size; `Some(0)`: one chunk, the whole file hashed.
    header: Option<usize>,
    /// Chunks: (compressed size, MD5), and the one being read.
    chunks: Vec<(usize, [u8; 16])>,
    at: usize,
    /// Bytes of the current chunk read so far, and their digest.
    read: usize,
    md5: Md5,
    ok: bool,
}

impl Verifier {
    #[must_use]
    pub fn new(ekey: [u8; 16]) -> Self {
        Self { ekey, head: Vec::new(), header: None, chunks: Vec::new(), at: 0, read: 0, md5: Md5::new(), ok: true }
    }

    /// The next bytes of the file.
    pub fn feed(&mut self, mut data: &[u8]) {
        while self.ok && !data.is_empty() {
            match self.header {
                None => data = self.take_head(data),
                Some(0) => {
                    self.md5.update(data);
                    return;
                }
                Some(_) => data = self.take_chunk(data),
            }
        }
    }

    /// Collects the header; once whole, checks it and reads its chunk table.
    fn take_head<'a>(&mut self, data: &'a [u8]) -> &'a [u8] {
        let want = if self.head.len() < 8 { 8 } else { be(&self.head[4..8]) as usize };
        let take = want.saturating_sub(self.head.len()).min(data.len());
        self.head.extend_from_slice(&data[..take]);
        let rest = &data[take..];
        if self.head.len() < 8 {
            return rest;
        }
        if &self.head[..4] != b"BLTE" {
            self.ok = false;
            return rest;
        }
        let size = be(&self.head[4..8]) as usize;
        if size == 0 {
            self.md5.update(&self.head);
            self.header = Some(0);
            return rest;
        }
        // Known bad before it is collected: no header this long or of this shape is held.
        if !(12..=MAX_HEADER).contains(&size) || !(size - 12).is_multiple_of(24) {
            self.ok = false;
            return rest;
        }
        if self.head.len() < size {
            return rest;
        }
        let h = &self.head;
        let n = if h.len() >= 12 { be(&h[9..12]) as usize } else { 0 };
        if h.len() < 12 || h[8] != 0x0F || n == 0 || 12 + n * 24 != size || md5(h) != self.ekey {
            self.ok = false;
            return rest;
        }
        self.chunks = h[12..]
            .chunks_exact(24)
            .map(|c| {
                let mut sum = [0u8; 16];
                sum.copy_from_slice(&c[8..24]);
                (be(&c[..4]) as usize, sum)
            })
            .collect();
        self.header = Some(size);
        rest
    }

    /// Hashes the current chunk's bytes; checks it once whole.
    fn take_chunk<'a>(&mut self, data: &'a [u8]) -> &'a [u8] {
        let Some(&(size, sum)) = self.chunks.get(self.at) else {
            self.ok = false; // bytes past the last chunk
            return &[];
        };
        let take = (size - self.read).min(data.len());
        self.md5.update(&data[..take]);
        self.read += take;
        if self.read == size {
            self.ok &= self.md5.finish() == sum;
            self.md5 = Md5::new();
            self.read = 0;
            self.at += 1;
        }
        &data[take..]
    }

    /// Whether the file was whole and intact.
    #[must_use]
    pub fn finish(&self) -> bool {
        match self.header {
            _ if !self.ok => false,
            Some(0) => self.md5.finish() == self.ekey,
            Some(_) => self.at == self.chunks.len() && self.read == 0,
            None => false,
        }
    }
}
