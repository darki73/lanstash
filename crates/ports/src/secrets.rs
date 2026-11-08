//! Small secrets kept between runs (a Steam refresh token), readable only by
//! the service's own user.

use alloc::string::String;

/// Named secrets.
pub trait Secrets: Send + Sync {
    /// The secret `name`, if one is kept.
    ///
    /// # Errors
    /// It exists but cannot be read.
    fn load(&self, name: &str) -> Result<Option<String>, String>;

    /// Keeps `value` as `name`, replacing what was there.
    ///
    /// # Errors
    /// It cannot be written.
    fn save(&self, name: &str, value: &str) -> Result<(), String>;

    /// Forgets `name` (nothing to do when it is not kept).
    ///
    /// # Errors
    /// It cannot be removed.
    fn remove(&self, name: &str) -> Result<(), String>;
}
