//! The settings that can be changed while lanstash runs (from the web UI):
//! what they are and where each value comes from, changing one, and the
//! restart that applies them.

use alloc::string::String;

/// Settings changed from outside the configuration file.
pub trait Tunables: Send + Sync {
    /// Every changeable setting, its value and where that comes from, as JSON.
    fn view(&self) -> String;

    /// Sets `key` to `value` (checked like the configuration file checks it);
    /// it applies at the next start.
    ///
    /// # Errors
    /// Not a changeable setting, set by a command-line flag, an invalid value,
    /// or it cannot be saved.
    fn set(&self, key: &str, value: &str) -> Result<(), String>;

    /// Drops the change of `key`: the configuration file's value counts again.
    ///
    /// # Errors
    /// Not a changeable setting, or it cannot be saved.
    fn reset(&self, key: &str) -> Result<(), String>;
}
