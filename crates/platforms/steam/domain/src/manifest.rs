//! A depot manifest, as the content servers send it
//! (`/depot/<depot>/manifest/<gid>/5/<code>`): a ZIP of one file holding
//! sections, each a `u32` magic and a `u32` length before a protobuf: the
//! payload (files and their chunks), the metadata, a signature, then an end
//! magic. What a depot's download is: its unique chunks, each fetched as
//! `/depot/<depot>/chunk/<sha1 hex>`, `compressed` bytes on the wire.
//!
//! Field numbers follow Steam's `content_manifest.proto`.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::zip::{ZipError, first};

use crate::cm::proto::{Wire, fields};

const PAYLOAD: u32 = 0x71F6_17D0;
const METADATA: u32 = 0x1F48_12BE;
const SIGNATURE: u32 = 0x1B81_B817;
const END: u32 = 0x32C4_15AB;

/// Largest unzipped manifest accepted (the largest games' are tens of MB).
pub const MAX_MANIFEST: usize = 512 << 20;

/// Why a manifest could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Zip(ZipError),
    /// A section ran past the end, or a magic is unknown.
    Sections,
    /// A protobuf that does not parse (which one).
    Malformed(&'static str),
}

/// One chunk: a piece of a file, compressed and encrypted on the CDN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Chunk {
    /// 1: SHA-1 of its contents (decrypted, uncompressed); its name on the CDN.
    pub sha: [u8; 20],
    /// 2: a checksum of its contents.
    pub crc: u32,
    /// 3: where it goes in its file.
    pub offset: u64,
    /// 4: its size unpacked.
    pub original: u32,
    /// 5: its size on the wire.
    pub compressed: u32,
}

impl Chunk {
    /// Its name on the CDN: the SHA-1 in lower-case hex.
    #[must_use]
    pub fn name(&self) -> String {
        use core::fmt::Write;
        let mut s = String::with_capacity(40);
        for b in self.sha {
            let _ = write!(s, "{b:02x}");
        }
        s
    }
}

/// One file of the depot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// 1: its name (encrypted, base64, when the metadata says so).
    pub name: String,
    /// 2.
    pub size: u64,
    /// 3: Steam's flags (directory, executable, ...).
    pub flags: u32,
    /// 6.
    pub chunks: Vec<Chunk>,
}

/// A manifest: its metadata and files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Metadata 1.
    pub depot: u32,
    /// Metadata 2.
    pub gid: u64,
    /// Metadata 3 (Unix seconds).
    pub created: u32,
    /// Metadata 4.
    pub names_encrypted: bool,
    /// Metadata 5, 6: bytes unpacked and on the wire.
    pub original: u64,
    pub compressed: u64,
    /// Metadata 7.
    pub unique_chunks: u32,
    pub files: Vec<File>,
}

fn sha(v: Wire<'_>) -> Option<[u8; 20]> {
    v.bytes()?.try_into().ok()
}

fn chunk(data: &[u8]) -> Result<Chunk, ManifestError> {
    let mut c = Chunk { sha: [0; 20], crc: 0, offset: 0, original: 0, compressed: 0 };
    for f in fields(data) {
        match f.map_err(|()| ManifestError::Malformed("chunk"))? {
            (1, v) => c.sha = sha(v).ok_or(ManifestError::Malformed("chunk sha"))?,
            (2, v) => c.crc = v.int().unwrap_or(0) as u32,
            (3, v) => c.offset = v.int().unwrap_or(0),
            (4, v) => c.original = v.int().unwrap_or(0) as u32,
            (5, v) => c.compressed = v.int().unwrap_or(0) as u32,
            _ => {}
        }
    }
    Ok(c)
}

fn file(data: &[u8]) -> Result<File, ManifestError> {
    let mut f = File { name: String::new(), size: 0, flags: 0, chunks: Vec::new() };
    for g in fields(data) {
        match g.map_err(|()| ManifestError::Malformed("file"))? {
            (1, v) => f.name = String::from_utf8_lossy(v.bytes().unwrap_or_default()).into_owned(),
            (2, v) => f.size = v.int().unwrap_or(0),
            (3, v) => f.flags = v.int().unwrap_or(0) as u32,
            (6, v) => f.chunks.push(chunk(v.bytes().unwrap_or_default())?),
            _ => {}
        }
    }
    Ok(f)
}

fn metadata(data: &[u8], m: &mut Manifest) -> Result<(), ManifestError> {
    for f in fields(data) {
        match f.map_err(|()| ManifestError::Malformed("metadata"))? {
            (1, v) => m.depot = v.int().unwrap_or(0) as u32,
            (2, v) => m.gid = v.int().unwrap_or(0),
            (3, v) => m.created = v.int().unwrap_or(0) as u32,
            (4, v) => m.names_encrypted = v.int() == Some(1),
            (5, v) => m.original = v.int().unwrap_or(0),
            (6, v) => m.compressed = v.int().unwrap_or(0),
            (7, v) => m.unique_chunks = v.int().unwrap_or(0) as u32,
            _ => {}
        }
    }
    Ok(())
}

/// Reads the sections of an unzipped manifest.
///
/// # Errors
/// [`ManifestError`].
pub fn parse_sections(data: &[u8]) -> Result<Manifest, ManifestError> {
    let mut m = Manifest::default();
    let mut at = 0usize;
    let word = |at: usize| -> Result<u32, ManifestError> {
        let b = data.get(at..at + 4).ok_or(ManifestError::Sections)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    loop {
        let magic = word(at)?;
        if magic == END {
            return Ok(m);
        }
        let len = word(at + 4)? as usize;
        let body = data.get(at + 8..at + 8 + len).ok_or(ManifestError::Sections)?;
        match magic {
            PAYLOAD => {
                for f in fields(body) {
                    if let (1, v) = f.map_err(|()| ManifestError::Malformed("payload"))? {
                        m.files.push(file(v.bytes().unwrap_or_default())?);
                    }
                }
            }
            METADATA => metadata(body, &mut m)?,
            SIGNATURE => {}
            _ => return Err(ManifestError::Sections),
        }
        at += 8 + len;
    }
}

/// Reads a manifest as the content servers send it (zipped).
///
/// # Errors
/// [`ManifestError`].
pub fn parse(zip: &[u8]) -> Result<Manifest, ManifestError> {
    parse_sections(&first(zip, MAX_MANIFEST).map_err(ManifestError::Zip)?)
}

impl Manifest {
    /// Every chunk once (files share chunks), by name.
    #[must_use]
    pub fn chunks(&self) -> Vec<Chunk> {
        let mut by: BTreeMap<[u8; 20], Chunk> = BTreeMap::new();
        for c in self.files.iter().flat_map(|f| &f.chunks) {
            by.entry(c.sha).or_insert(*c);
        }
        by.into_values().collect()
    }

    /// Bytes on the wire of every chunk once.
    #[must_use]
    pub fn download(&self) -> u64 {
        self.chunks().iter().map(|c| u64::from(c.compressed)).sum()
    }
}

/// A chunk's path on the CDN (and in lancache's keys).
#[must_use]
pub fn chunk_path(depot: u32, chunk: &Chunk) -> String {
    alloc::format!("/depot/{depot}/chunk/{}", chunk.name())
}
