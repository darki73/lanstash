//! The chunk lists of depots' current manifests, kept between runs ([`Keep`],
//! one blob per depot, `steam-depot-<depot>`), so a restart need not download
//! every stashed game's manifests again: a manifest never changes under its id
//! (gid), so a list kept for the gid Steam names is that manifest's. A depot
//! keeps only its latest gid's list; a new build replaces it. A blob is the
//! magic `LSD1`, the gid, the count, then per chunk its SHA-1, CRC, size
//! unpacked and size on the wire (32 bytes; the chunk's place in a file is not
//! kept: a list is of unique chunks).

use std::sync::Arc;

use lanstash_ports::keep::Keep;
use lanstash_steam_domain::manifest::Chunk;

const MAGIC: &[u8; 4] = b"LSD1";
const CHUNK: usize = 32;

/// The blob name of `depot`'s list.
#[must_use]
pub fn name(depot: u32) -> String {
    format!("steam-depot-{depot}")
}

/// `chunks` of manifest `gid`, as a blob.
#[must_use]
pub fn encode(gid: u64, chunks: &[Chunk]) -> Vec<u8> {
    let mut b = Vec::with_capacity(16 + CHUNK * chunks.len());
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(&gid.to_le_bytes());
    b.extend_from_slice(&u32::try_from(chunks.len()).unwrap_or(u32::MAX).to_le_bytes());
    for c in chunks {
        b.extend_from_slice(&c.sha);
        b.extend_from_slice(&c.crc.to_le_bytes());
        b.extend_from_slice(&c.original.to_le_bytes());
        b.extend_from_slice(&c.compressed.to_le_bytes());
    }
    b
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A blob's gid and chunks; none when it is not a whole one.
#[must_use]
pub fn decode(b: &[u8]) -> Option<(u64, Vec<Chunk>)> {
    if b.get(..4)? != MAGIC {
        return None;
    }
    let gid = u64::from_le_bytes(b.get(4..12)?.try_into().ok()?);
    let n = u32_at(b, 12)? as usize;
    if b.len() != 16 + n * CHUNK {
        return None;
    }
    let chunks = b[16..]
        .chunks_exact(CHUNK)
        .map(|r| Chunk {
            sha: r[..20].try_into().unwrap_or([0; 20]),
            crc: u32_at(r, 20).unwrap_or(0),
            offset: 0,
            original: u32_at(r, 24).unwrap_or(0),
            compressed: u32_at(r, 28).unwrap_or(0),
        })
        .collect();
    Some((gid, chunks))
}

/// The kept lists (none kept without a [`Keep`]).
#[derive(Clone, Default)]
pub struct ChunkLists {
    keep: Option<Arc<dyn Keep>>,
}

impl std::fmt::Debug for ChunkLists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkLists").field("kept", &self.keep.is_some()).finish()
    }
}

impl ChunkLists {
    #[must_use]
    pub fn new(keep: Arc<dyn Keep>) -> Self {
        Self { keep: Some(keep) }
    }

    /// The kept list of `depot`'s manifest `gid`, if that is the one kept.
    #[must_use]
    pub fn kept(&self, depot: u32, gid: u64) -> Option<Vec<Chunk>> {
        let b = self.keep.as_ref()?.load(&name(depot)).ok()??;
        decode(&b).filter(|(g, _)| *g == gid).map(|(_, c)| c)
    }

    /// Keeps `chunks` as `depot`'s list of manifest `gid`. Losing it costs a download, never correctness.
    pub fn put(&self, depot: u32, gid: u64, chunks: &[Chunk]) {
        if let Some(k) = &self.keep {
            let _ = k.save(&name(depot), &encode(gid, chunks));
        }
    }

    /// Drops `depot`'s list (its game is deleted). Losing it costs a download, never correctness.
    pub fn forget(&self, depot: u32) {
        if let Some(k) = &self.keep {
            let _ = k.remove(&name(depot));
        }
    }
}
