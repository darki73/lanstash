//! Reading a file out of a ZIP archive (PKWARE APPNOTE): the central
//! directory names the entries, each local header leads to the data, stored
//! or deflated ([`crate::inflate`]), checked by its CRC-32. Steam's depot
//! manifests come as a ZIP of one deflated file. No ZIP64, no encryption.

use alloc::string::String;
use alloc::vec::Vec;

use crate::checksum::crc::crc32;
use crate::inflate::{InflateError, inflate};

/// Why an entry could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipError {
    /// No end-of-central-directory record, or a record out of bounds.
    NotZip,
    /// No such entry.
    Missing,
    /// A method other than stored (0) or deflate (8).
    Method(u16),
    Inflate(InflateError),
    /// The data does not match its CRC-32 or its size.
    BadChecksum,
    /// Larger than the caller's limit.
    TooLarge,
}

/// One entry of the central directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub method: u16,
    pub crc: u32,
    pub compressed: u32,
    pub size: u32,
    /// Where its local header is.
    pub offset: u32,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*d.get(at)?, *d.get(at + 1)?, *d.get(at + 2)?, *d.get(at + 3)?]))
}

/// The entries of `zip`, from its central directory.
///
/// # Errors
/// [`ZipError::NotZip`].
pub fn entries(zip: &[u8]) -> Result<Vec<Entry>, ZipError> {
    // The end record is 22 bytes plus a comment of at most 64 KiB.
    let low = zip.len().saturating_sub(22 + 0xFFFF);
    let end = (low..=zip.len().saturating_sub(22))
        .rev()
        .find(|&i| u32_at(zip, i) == Some(0x0605_4b50))
        .ok_or(ZipError::NotZip)?;
    let count = u16_at(zip, end + 10).ok_or(ZipError::NotZip)?;
    let mut at = u32_at(zip, end + 16).ok_or(ZipError::NotZip)? as usize;
    let mut out = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        if u32_at(zip, at) != Some(0x0201_4b50) {
            return Err(ZipError::NotZip);
        }
        let field16 = |o: usize| u16_at(zip, at + o).ok_or(ZipError::NotZip);
        let field32 = |o: usize| u32_at(zip, at + o).ok_or(ZipError::NotZip);
        let (name_len, extra_len, comment_len) =
            (usize::from(field16(28)?), usize::from(field16(30)?), usize::from(field16(32)?));
        let name = zip.get(at + 46..at + 46 + name_len).ok_or(ZipError::NotZip)?;
        out.push(Entry {
            name: String::from_utf8_lossy(name).into_owned(),
            method: field16(10)?,
            crc: field32(16)?,
            compressed: field32(20)?,
            size: field32(24)?,
            offset: field32(42)?,
        });
        at += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}

/// The contents of `entry`, at most `max` bytes.
///
/// # Errors
/// [`ZipError`].
pub fn read(zip: &[u8], entry: &Entry, max: usize) -> Result<Vec<u8>, ZipError> {
    if entry.size as usize > max {
        return Err(ZipError::TooLarge);
    }
    let at = entry.offset as usize;
    if u32_at(zip, at) != Some(0x0403_4b50) {
        return Err(ZipError::NotZip);
    }
    let name_len = usize::from(u16_at(zip, at + 26).ok_or(ZipError::NotZip)?);
    let extra_len = usize::from(u16_at(zip, at + 28).ok_or(ZipError::NotZip)?);
    let start = at + 30 + name_len + extra_len;
    let data = zip.get(start..start + entry.compressed as usize).ok_or(ZipError::NotZip)?;
    let out = match entry.method {
        0 => data.to_vec(),
        8 => inflate(data, max).map_err(ZipError::Inflate)?.0,
        m => return Err(ZipError::Method(m)),
    };
    if out.len() != entry.size as usize || crc32(&out) != entry.crc {
        return Err(ZipError::BadChecksum);
    }
    Ok(out)
}

/// The contents of the first entry (Steam's manifests hold one).
///
/// # Errors
/// [`ZipError`]; `Missing` for an empty archive.
pub fn first(zip: &[u8], max: usize) -> Result<Vec<u8>, ZipError> {
    let e = entries(zip)?.into_iter().next().ok_or(ZipError::Missing)?;
    read(zip, &e, max)
}
