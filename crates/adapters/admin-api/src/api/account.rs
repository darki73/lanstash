//! The account endpoints: a platform's accounts and their QR sign-in.
//!
//! - `GET /api/v1/account/<platform>`: the accounts view (who is signed in,
//!   the sign-in's state),
//! - `GET .../qr.svg`: the waiting sign-in's QR code (404 when none waits),
//! - `POST .../sign-in`: start a QR sign-in that adds an account (202); with a code
//!   pasted from the platform's own page in [`SECRET_HEADER`] (or `?code=`): sign in
//!   with it (200; 400 when refused),
//!   `DELETE .../sign-in`: cancel it, `POST .../sign-out?account=<name>`:
//!   forget that account's kept sign-in (400 without a name),
//! - `POST .../client` with `<id>:<secret>` in [`SECRET_HEADER`] (or `?value=`): the client the
//!   platform's sign-in names in place of the built-in one, `DELETE .../client`: back to the
//!   built-in one (400 when the platform has no such choice or the value is not one).
//!
//! What signs in travels in a header, not in the path, which logs and histories keep.

use std::sync::Arc;

use lanstash_ports::account::Account;

use super::dns::query;
use super::settings::decode;
use super::{AdminApi, Answer};
use crate::route::{ACCOUNT, Route, rest};

/// The header a sign-in code or a client's secret comes in.
pub const SECRET_HEADER: &str = "x-lanstash-secret";

impl AdminApi {
    /// The platform's account and what the path asks of it.
    fn account_of<'a>(&self, target: &'a str) -> Option<(Arc<dyn Account>, &'a str)> {
        let rest = rest(target, ACCOUNT);
        let (id, what) = rest.split_once('/').unwrap_or((rest, ""));
        Some((self.platform(id)?.account()?, what))
    }

    /// `/api/v1/account/<platform>[/...]` for an [`Route::Account`] route; `secret`: the
    /// [`SECRET_HEADER`] sent with it.
    pub(super) fn account(&self, route: Route, target: &str, secret: Option<&str>) -> Answer {
        let Some((account, what)) = self.account_of(target) else {
            return Answer::error(target, 404, "not-found", "no account for this platform");
        };
        // Percent-encoded in the header too (a pasted page may hold line breaks).
        let given =
            |name: &str| secret.filter(|s| !s.trim().is_empty()).and_then(decode).or_else(|| query(target, name));
        match (route, what) {
            (Route::Account, "") => Answer::json(200, account.view()),
            (Route::Account, "qr.svg") => match account.qr_svg() {
                Some(svg) => Answer::new(200, "image/svg+xml", svg.into_bytes()),
                None => Answer::error(target, 404, "not-found", "no sign-in is waiting"),
            },
            (Route::AccountPost, "sign-in") => match given("code") {
                // A code pasted from the platform's own page (Epic): the account is added now.
                Some(code) => match account.sign_in_with_code(&code) {
                    Ok(()) => Answer::json(200, account.view()),
                    Err(e) => Answer::error(target, 400, "refused", &e),
                },
                None => match account.start_sign_in() {
                    Ok(()) => Answer::json(202, account.view()),
                    Err(e) => Answer::error(target, 500, "internal", &e),
                },
            },
            (Route::AccountDelete, "sign-in") => {
                account.cancel_sign_in();
                Answer::json(200, account.view())
            }
            (Route::AccountPost, "sign-out") => match query(target, "account") {
                None => Answer::error(target, 400, "bad-request", "which account: ?account=<name>"),
                Some(name) => match account.sign_out(&name) {
                    Ok(()) => Answer::json(200, account.view()),
                    Err(e) => Answer::error(target, 500, "internal", &e),
                },
            },
            (Route::AccountPost | Route::AccountDelete, "client") => {
                let value = given("value");
                let set = if route == Route::AccountDelete { None } else { value.as_deref() };
                if route == Route::AccountPost && set.is_none() {
                    return Answer::error(target, 400, "bad-request", "which client: <id>:<secret>");
                }
                match account.set_client(set) {
                    Ok(()) => Answer::json(200, account.view()),
                    Err(e) => Answer::error(target, 400, "bad-request", &e),
                }
            }
            _ => Answer::error(target, 404, "not-found", "not found"),
        }
    }
}
