//! The setup endpoint: `GET /api/v1/setup/<platform>/<os>`, the file that
//! sets up a PC running `os` (`windows`, `macos`, `linux`) for a platform
//! ([`lanstash_ports::platform::Platform::setup`]), sent to save under its
//! name; 404 where the platform makes none.

use lanstash_ports::platform::ClientOs;

use super::{AdminApi, Answer};
use crate::route::{SETUP, rest};

impl AdminApi {
    /// `/api/v1/setup/<platform>/<os>`.
    pub(super) fn setup(&self, target: &str) -> Answer {
        let file =
            rest(target, SETUP).split_once('/').and_then(|(id, os)| self.platform(id)?.setup(ClientOs::parse(os)?));
        match file {
            Some(f) => Answer::download(f.content_type, &f.name, f.body),
            None => Answer::error(target, 404, "not-found", "no setup file for this platform and system"),
        }
    }
}
