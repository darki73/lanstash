//! The file size rule: every Rust file of a crate stays within a line ceiling.

use crate::violation::Violation;
use crate::workspace::{Crate, slash};

/// Checks every `.rs` file under `src/` and `tests/` of `krate` against `max_lines`;
/// `exempt` lists paths from the workspace root (`crates/app/src/serve.rs`) that may be longer.
#[must_use]
pub fn check_file_sizes(krate: &Crate, max_lines: usize, exempt: &[&str]) -> Vec<Violation> {
    let sources = krate.sources.iter().map(|f| ("src", f));
    let tests = krate.tests.iter().map(|f| ("tests", f));
    let mut violations = Vec::new();
    for (dir, file) in sources.chain(tests) {
        let path = format!("{}/{dir}/{}", krate.dir, slash(&file.relative));
        let lines = file.text.lines().count();
        if lines > max_lines && !exempt.contains(&path.as_str()) {
            violations.push(Violation::new(
                krate.name(),
                "file-size",
                format!("{path} has {lines} lines (limit {max_lines})"),
            ));
        }
    }
    violations
}
