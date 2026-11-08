//! Small state kept between runs so work need not be redone after a restart
//! (what lancache's nginx cache holds of each Steam game, say). Losing it costs
//! time, never correctness.

use alloc::string::String;
use alloc::vec::Vec;

/// Named blobs.
pub trait Keep: Send + Sync {
    /// The blob `name`, if one is kept.
    ///
    /// # Errors
    /// It exists but cannot be read.
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String>;

    /// Keeps `data` as `name`, replacing what was there.
    ///
    /// # Errors
    /// It cannot be written.
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String>;

    /// Forgets `name` (nothing to do when it is not kept; a store that cannot forget does nothing).
    ///
    /// # Errors
    /// It is kept but cannot be removed.
    fn remove(&self, name: &str) -> Result<(), String> {
        let _ = name;
        Ok(())
    }
}
