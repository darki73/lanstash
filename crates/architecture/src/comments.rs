//! Comment hygiene: no em-dashes anywhere in the Rust, TOML and Markdown files under `crates/`.

use crate::violation::Violation;

const EM_DASH: char = '\u{2014}';

/// Reports every line of `text` (the file at `path`) holding an em-dash.
#[must_use]
pub fn check_em_dashes(path: &str, text: &str) -> Vec<Violation> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(EM_DASH))
        .map(|(i, _)| Violation::new(path, "comment-style", format!("em-dash at {path}:{}", i + 1)))
        .collect()
}
