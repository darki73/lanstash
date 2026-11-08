//! A whole answer of the admin API, before it is sent: status, content type,
//! extra headers, body; and the usual kinds (JSON, text, an error, a file to save,
//! a platform's view).

use lanstash_ports::platform::{View, ViewError};

use crate::render::error_json;
use crate::route::is_api;

/// A whole answer: status, content type, extra headers, body.
pub(super) struct Answer {
    pub(super) status: u16,
    pub(super) ctype: String,
    pub(super) allow: Option<&'static str>,
    pub(super) cache: &'static str,
    /// `Content-Disposition`: a file to save, under its name.
    pub(super) disposition: Option<String>,
    /// `Set-Cookie`: a session given or ended.
    pub(super) cookie: Option<String>,
    pub(super) body: Vec<u8>,
}

impl Answer {
    pub(super) fn new(status: u16, ctype: &str, body: Vec<u8>) -> Self {
        Self { status, ctype: ctype.to_owned(), allow: None, cache: "no-store", disposition: None, cookie: None, body }
    }

    /// A file to save as `name` (letters, digits, spaces and `()._-` only reach the header).
    pub(super) fn download(ctype: &str, name: &str, body: Vec<u8>) -> Self {
        let safe: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '(' | ')' | '.' | '_' | '-'))
            .collect();
        Self { disposition: Some(format!("attachment; filename=\"{safe}\"")), ..Self::new(200, ctype, body) }
    }

    pub(super) fn json(status: u16, body: String) -> Self {
        Self::new(status, "application/json", body.into_bytes())
    }

    pub(super) fn text(status: u16, body: &str) -> Self {
        Self::new(status, "text/plain", body.as_bytes().to_vec())
    }

    /// An error: JSON on an API path, a line of text elsewhere.
    pub(super) fn error(target: &str, status: u16, kind: &str, message: &str) -> Self {
        if is_api(target) {
            Self::json(status, error_json(kind, message))
        } else {
            Self::text(status, &format!("{message}\n"))
        }
    }

    /// A platform's view; `missing` says why there is none.
    pub(super) fn view(target: &str, view: Option<View>, missing: &str) -> Self {
        match view {
            Some(Ok(body)) => Self::json(200, body),
            Some(Err(ViewError::NotFound(e))) => Self::error(target, 404, "not-found", &e),
            Some(Err(ViewError::Unavailable(e))) => Self::error(target, 503, "unavailable", &e),
            None => Self::error(target, 404, "not-found", missing),
        }
    }
}
