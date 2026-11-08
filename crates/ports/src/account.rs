//! A platform's accounts lanstash signs in to (Steam, Epic): their state as a
//! JSON view, a sign-in by QR code that runs in the background and adds the
//! account that approves it (Steam), or by a code pasted from the platform's own
//! page (Epic), and signing one out. Several may be signed in at once; what
//! one owns, lanstash fetches as that one.

use alloc::string::String;

/// One platform's accounts.
pub trait Account: Send + Sync {
    /// `{"accounts": [{"name": <name>, "refused": <bool>}, ...], "account": <the first's name or
    /// null>, "sign_in": {"state": "idle" | "starting" | "waiting" | "failed", "scanned": <bool>,
    /// "error": <text or null>}}`; `refused`: the platform refuses that account's kept sign-in.
    /// A platform that signs in with a pasted code adds `"method": "code"` and the page to sign in
    /// on (`"login_url"`), each account's `"id"` (what signing it out takes), and which client
    /// its sign-in names (`"client": "built-in" | "own"`).
    fn view(&self) -> String;

    /// Starts a QR sign-in (a new code each time; a waiting one is replaced). The account
    /// that approves it is added (or its sign-in renewed, when it is there already).
    ///
    /// # Errors
    /// It cannot be started (a thread could not be spawned).
    fn start_sign_in(&self) -> Result<(), String>;

    /// Stops a waiting sign-in (nothing to do when none waits).
    fn cancel_sign_in(&self);

    /// Signs in with what a person pasted from the platform's own sign-in page (Epic: the
    /// one-time code it shows); the account it is for is added (or renewed).
    ///
    /// # Errors
    /// The platform signs in otherwise (the default), or refused the code.
    fn sign_in_with_code(&self, _pasted: &str) -> Result<(), String> {
        Err("this platform signs in by QR code".into())
    }

    /// Sets the client the platform's sign-in names (`<id>:<secret>`), or goes back to the
    /// built-in one (`None`). Signed-in accounts keep their sign-ins.
    ///
    /// # Errors
    /// The platform has no such choice (the default), the value is not a client, or it cannot be kept.
    fn set_client(&self, _client: Option<&str>) -> Result<(), String> {
        Err("this platform's sign-in has no client to choose".into())
    }

    /// Forgets `account`'s kept sign-in.
    ///
    /// # Errors
    /// It is not signed in, or the secret could not be removed.
    fn sign_out(&self, account: &str) -> Result<(), String>;

    /// The waiting sign-in's QR code as an SVG image.
    fn qr_svg(&self) -> Option<String>;
}
