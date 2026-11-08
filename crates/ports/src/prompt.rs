//! Asking the person at the keyboard (an account name, a password, a Steam
//! Guard code).

use alloc::string::String;

/// Asks questions and returns the answers.
pub trait Prompt: Send + Sync {
    /// Shows `question` and returns the answer, without its line end; a
    /// `secret` answer is not echoed.
    ///
    /// # Errors
    /// No one can answer (no terminal, end of input).
    fn ask(&self, question: &str, secret: bool) -> Result<String, String>;

    /// Shows `message` (no answer expected).
    fn tell(&self, message: &str);
}
