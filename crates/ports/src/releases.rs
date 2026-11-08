//! Where lanstash releases are published, for `lanstash doctor`.

use alloc::string::String;

/// A release source.
pub trait Releases: Send + Sync {
    /// The newest released version, `None` when this source does not know.
    ///
    /// # Errors
    /// The source could not be asked.
    fn latest(&self) -> Result<Option<String>, String>;
}

/// No release source configured.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoReleases;

impl Releases for NoReleases {
    fn latest(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
}
