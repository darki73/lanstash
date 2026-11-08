//! The built web UI (`web/dist`), when a directory of it is given ([`AdminApi::with_web`]):
//! `/` and every path of the page's own (`/platforms/steam`, ...) answer its `index.html`, which
//! draws the page for its path; a file of it (`/assets/<name>.<hash>.js`, an icon, the web app
//! manifest) is answered as itself, a hashed asset kept by the browser for good, the rest for an
//! hour. Without one, `/` is the built-in status page. API paths never come here.

use std::path::{Path, PathBuf};

use super::{AdminApi, Answer, PAGE};

/// The content type of a file of the UI, by its extension.
fn content_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map_or("", |(_, e)| e) {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// The file `path` names inside `dir`, when every part of it is a plain name.
fn file_of(dir: &Path, path: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let plain = |p: &&str| {
        !p.is_empty() && !p.starts_with('.') && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    parts.iter().all(plain).then(|| parts.iter().fold(dir.to_path_buf(), |d, p| d.join(p)))
}

impl AdminApi {
    /// Serves the built web UI from `dir` (it holds `index.html` and `assets/`).
    #[must_use]
    pub fn with_web(mut self, dir: PathBuf) -> Self {
        self.web = Some(dir);
        self
    }

    /// `/`: the web UI's page, else the built-in status page.
    pub(super) fn page(&self, target: &str) -> Answer {
        self.web_file(target, true)
            .unwrap_or_else(|| Answer::new(200, "text/html; charset=utf-8", PAGE.as_bytes().to_vec()))
    }

    /// A path no route takes: a file of the web UI, or a path of its own (its page), else 404.
    pub(super) fn not_found(&self, target: &str) -> Answer {
        self.web_file(target, false).unwrap_or_else(|| Answer::error(target, 404, "not-found", "not found"))
    }

    /// What the web UI answers for `target`; `page`: its `index.html` whatever the path.
    fn web_file(&self, target: &str, page: bool) -> Option<Answer> {
        let dir = self.web.as_ref()?;
        let path = target.split(['?', '#']).next().unwrap_or(target);
        if crate::route::is_api(path) {
            return None;
        }
        let last = path.rsplit('/').next().unwrap_or("");
        // A name with an extension is a file of the UI; a path without one is a page of it.
        // A hashed asset never changes under its name; the icons and the manifest may.
        let (file, cache) = if !page && last.contains('.') {
            let cache = if path.starts_with("/assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "public, max-age=3600"
            };
            (file_of(dir, path)?, cache)
        } else {
            (dir.join("index.html"), "no-cache")
        };
        let body = std::fs::read(&file).ok()?;
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        Some(Answer { cache, ..Answer::new(200, content_type(name), body) })
    }
}
