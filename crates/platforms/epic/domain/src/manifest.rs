//! An Epic build manifest, binary form (**verified** on Epic's manifests: a build's
//! chunks listed and every one fetched with the size and hash listed), and the
//! chunks a download of the build takes ([`parse`]). A manifest is a header
//! (magic `0x44BEC00C`, its size, the data's sizes, a SHA-1, how the data is
//! stored, a version) and data, zlib-compressed when the header says so: the
//! build's meta (skipped by its size), then the chunk list, column by column:
//! each chunk's GUID, rolling hash, SHA-1, group, window size and file size.
//! A chunk is downloaded from `<the manifest's directory>/<data dir>/<group,
//! two digits>/<hash, 16 hex digits>_<GUID, 32 hex digits>.chunk` ([`Chunk::path`]).
//! A chunk file ([`check_chunk`]) is a header (magic `0xB1FE3AA2`, its size,
//! the data's size, how it is stored) and the data, zlib-compressed when the
//! header says so; its SHA-1 is the manifest's.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::checksum::sha1::sha1;
use lanstash_domain::inflate::zlib;

const MANIFEST_MAGIC: u32 = 0x44BE_C00C;
const CHUNK_MAGIC: u32 = 0xB1FE_3AA2;
/// The most a manifest's data or a chunk's data may inflate to.
const MAX_DATA: usize = 256 << 20;

/// One chunk of a build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub guid: [u32; 4],
    pub hash: u64,
    pub sha1: [u8; 20],
    pub group: u8,
    /// Its size on the CDN.
    pub file_size: u64,
}

impl Chunk {
    /// Where it is, under the manifest's directory, for a manifest of `feature_level`.
    #[must_use]
    pub fn path(&self, feature_level: u32) -> String {
        let dir = match feature_level {
            15.. => "ChunksV4",
            6.. => "ChunksV3",
            3.. => "ChunksV2",
            _ => "Chunks",
        };
        let g = self.guid;
        format!("{dir}/{:02}/{:016X}_{:08X}{:08X}{:08X}{:08X}.chunk", self.group, self.hash, g[0], g[1], g[2], g[3])
    }
}

/// A build's chunks, and the manifest's feature level (which names their directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub feature_level: u32,
    pub chunks: Vec<Chunk>,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.at..self.at + n).ok_or("the manifest ends early")?;
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| "u32")?))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(|_| "u64")?))
    }
}

/// The chunks of a binary manifest.
///
/// # Errors
/// Not a binary manifest (a JSON one included), a bad SHA-1, or one cut short.
pub fn parse(b: &[u8]) -> Result<Manifest, String> {
    if b.first() == Some(&b'{') {
        return Err("a JSON manifest: not read yet".into());
    }
    let mut r = Reader { b, at: 0 };
    if r.u32()? != MANIFEST_MAGIC {
        return Err("not an Epic manifest".into());
    }
    let header_size = r.u32()? as usize;
    let size = r.u32()? as usize;
    let _compressed_size = r.u32()?;
    let sha: [u8; 20] = r.take(20)?.try_into().map_err(|_| "sha")?;
    let stored_as = r.u8()?;
    let body = b.get(header_size..).ok_or("the manifest ends early")?;
    let data = if stored_as & 1 == 1 {
        zlib(body, size.min(MAX_DATA)).map_err(|e| format!("manifest data: {e:?}"))?
    } else {
        body.to_vec()
    };
    if sha1(&data) != sha {
        return Err("the manifest's data does not match its SHA-1".into());
    }
    let mut d = Reader { b: &data, at: 0 };
    // The build's meta: its size first; only its feature level is needed.
    let meta_size = d.u32()? as usize;
    let _version = d.u8()?;
    let feature_level = d.u32()?;
    d.at = meta_size;
    let list_start = d.at;
    let list_size = d.u32()? as usize;
    let _list_version = d.u8()?;
    let count = d.u32()? as usize;
    if count > 1 << 22 {
        return Err("too many chunks".into());
    }
    let guids: Vec<[u32; 4]> =
        (0..count).map(|_| Ok([d.u32()?, d.u32()?, d.u32()?, d.u32()?])).collect::<Result<_, String>>()?;
    let hashes: Vec<u64> = (0..count).map(|_| d.u64()).collect::<Result<_, String>>()?;
    let shas: Vec<[u8; 20]> = (0..count)
        .map(|_| d.take(20).and_then(|s| s.try_into().map_err(|_| "sha".into())))
        .collect::<Result<_, String>>()?;
    let groups: Vec<u8> = (0..count).map(|_| d.u8()).collect::<Result<_, String>>()?;
    let _windows: Vec<u32> = (0..count).map(|_| d.u32()).collect::<Result<_, String>>()?;
    let sizes: Vec<u64> = (0..count).map(|_| d.u64()).collect::<Result<_, String>>()?;
    if d.at > list_start + list_size {
        return Err("the chunk list overruns its size".into());
    }
    let chunks = (0..count)
        .map(|i| Chunk { guid: guids[i], hash: hashes[i], sha1: shas[i], group: groups[i], file_size: sizes[i] })
        .collect();
    Ok(Manifest { feature_level, chunks })
}

/// Whether a chunk file (as the CDN sends it) holds data whose SHA-1 is `sha`.
#[must_use]
pub fn check_chunk(file: &[u8], sha: &[u8; 20]) -> bool {
    let mut r = Reader { b: file, at: 0 };
    let head = (|| -> Result<(usize, u8), String> {
        if r.u32()? != CHUNK_MAGIC {
            return Err("magic".into());
        }
        let _version = r.u32()?;
        let header_size = r.u32()? as usize;
        let _compressed = r.u32()?;
        r.take(16)?; // the GUID
        let _rolling = r.u64()?;
        let stored_as = r.u8()?;
        Ok((header_size, stored_as))
    })();
    let Ok((header_size, stored_as)) = head else { return false };
    let Some(body) = file.get(header_size..) else { return false };
    let data = if stored_as & 1 == 1 {
        match zlib(body, MAX_DATA) {
            Ok(d) => d,
            Err(_) => return false,
        }
    } else {
        body.to_vec()
    };
    &sha1(&data) == sha
}
