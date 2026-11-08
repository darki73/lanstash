//! One file of an nginx (lancache) cache, read through the storage ports:
//! its head parsed ([`read_cache_file`]), its stored response as an upstream
//! head ([`response_head`]) and as the headers lanstash keeps ([`meta`]).

use std::fmt;

use lanstash_domain::http::{ResponseHeadIn, STORED_RESPONSE_HEADERS};
use lanstash_domain::nginx_cache::{self, NginxCacheFile};
use lanstash_domain::segment::meta::encode_meta;
use lanstash_ports::storage::FileRead;

/// Why a cache file could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileError {
    /// This cache file is unusable; count it and continue.
    Bad(String),
    /// Writing the store failed; stop.
    Fatal(String),
}

impl FileError {
    /// What went wrong, in words.
    #[must_use]
    pub fn message(self) -> String {
        match self {
            Self::Bad(m) | Self::Fatal(m) => m,
        }
    }
}

pub(crate) fn bad(e: impl fmt::Display) -> FileError {
    FileError::Bad(e.to_string())
}

pub(crate) fn fatal(e: impl fmt::Display) -> FileError {
    FileError::Fatal(e.to_string())
}

/// Reads and parses the cache file head (everything before the body).
///
/// # Errors
/// [`FileError::Bad`]: too small, unreadable or not an nginx cache file.
pub fn read_cache_file(f: &dyn FileRead) -> Result<NginxCacheFile, FileError> {
    let size = f.size().map_err(bad)?;
    let mut prefix = [0u8; nginx_cache::PREFIX_LEN];
    if size < prefix.len() as u64 {
        return Err(FileError::Bad(format!("file too small ({size} bytes)")));
    }
    f.read_exact_at(0, &mut prefix).map_err(bad)?;
    let body_start = usize::from(nginx_cache::body_start(&prefix).map_err(bad)?);
    let mut head = vec![0u8; body_start];
    f.read_exact_at(0, &mut head).map_err(bad)?;
    nginx_cache::parse(&head, size).map_err(bad)
}

/// The stored response headers as an upstream head (for the profile's checks).
#[must_use]
pub fn response_head(f: &NginxCacheFile) -> ResponseHeadIn {
    ResponseHeadIn {
        status: f.status,
        minor_version: 1,
        headers: f.headers.iter().map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_owned())).collect(),
    }
}

/// The stored response headers lanstash keeps, encoded as record metadata.
#[must_use]
pub fn meta(f: &NginxCacheFile) -> Vec<u8> {
    let head = response_head(f);
    let pairs: Vec<(&str, &str)> = head
        .headers
        .iter()
        .filter(|(n, _)| STORED_RESPONSE_HEADERS.contains(&n.as_str()))
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    encode_meta(&pairs)
}
