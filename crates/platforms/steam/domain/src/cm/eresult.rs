//! Steam's result codes (`EResult`), the ones a logon meets, named so a
//! person can read them (numbers from Steam's `EResult`).

/// The code that means success.
pub const OK: i32 = 1;
pub const INVALID_PASSWORD: i32 = 5;
pub const ACCESS_DENIED: i32 = 15;
pub const REVOKED: i32 = 26;
pub const EXPIRED: i32 = 27;
pub const TRY_ANOTHER_CM: i32 = 48;
pub const SERVICE_UNAVAILABLE: i32 = 20;
pub const ACCOUNT_LOGON_DENIED: i32 = 63;
pub const INVALID_LOGIN_AUTH_CODE: i32 = 65;
pub const RATE_LIMIT_EXCEEDED: i32 = 84;
pub const TWO_FACTOR_CODE_MISMATCH: i32 = 88;

/// Whether a logon answered with `code` says the token itself will not do (or that
/// Steam wants no more tries for a while): trying it again soon only costs logons.
/// Any other failure (busy, another server, no connection) passes.
#[must_use]
pub fn refuses_token(code: i32) -> bool {
    matches!(
        code,
        INVALID_PASSWORD
            | ACCESS_DENIED
            | REVOKED
            | EXPIRED
            | ACCOUNT_LOGON_DENIED
            | INVALID_LOGIN_AUTH_CODE
            | RATE_LIMIT_EXCEEDED
            | TWO_FACTOR_CODE_MISMATCH
    )
}

/// A short description of `code`, for messages.
#[must_use]
pub fn describe(code: i32) -> &'static str {
    match code {
        OK => "OK",
        2 => "failed",
        3 => "no connection",
        INVALID_PASSWORD => "wrong account name or password",
        8 => "invalid parameter",
        ACCESS_DENIED => "access denied",
        SERVICE_UNAVAILABLE => "Steam is busy, try again",
        REVOKED => "revoked",
        EXPIRED => "expired",
        TRY_ANOTHER_CM => "try another server",
        ACCOUNT_LOGON_DENIED => "Steam Guard wants a code",
        INVALID_LOGIN_AUTH_CODE => "wrong Steam Guard code",
        RATE_LIMIT_EXCEEDED => "too many attempts; wait a while",
        TWO_FACTOR_CODE_MISMATCH => "wrong authenticator code",
        _ => "an unexpected result",
    }
}
