//! Turning a password into what is kept of it (a slow, salted hash), and checking a password
//! against what was kept, without ever keeping the password itself.

use alloc::string::String;

/// Hashes and checks passwords.
pub trait Passwords: Send + Sync {
    /// What is kept of `password`: a new salt and the hash, as one line of text without spaces.
    ///
    /// # Errors
    /// No salt could be drawn.
    fn hash(&self, password: &str) -> Result<String, String>;

    /// Whether `password` is the one `kept` was made from (false for what no hash of this kind
    /// is). It takes about as long either way.
    fn verify(&self, password: &str, kept: &str) -> bool;
}
