//! Tests mirror the source tree: every test file maps to a source module, and
//! every source module but crate roots and `mod.rs` files has a test file.
//!
//! Mapping: `tests/a.rs`, `tests/a/main.rs` and `tests/a/mod.rs` test `src/a.rs`
//! or `src/a/`; `tests/a/b.rs` tests `src/a/b.rs` or `src/a/b/`; the tests of a
//! module may be split by concern into files below its directory. Helpers in a
//! `support` directory or `support.rs` file at any depth are not test modules.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::violation::Violation;
use crate::workspace::{Crate, slash};

/// Checks `krate`; `untested` lists module paths (`origin`, `segment/bytes`) that need no test file.
#[must_use]
pub fn check_test_mirror(krate: &Crate, untested: &[&str]) -> Vec<Violation> {
    let modules: BTreeSet<PathBuf> = krate.sources.iter().filter_map(|f| module_path(&f.relative)).collect();
    let tested: BTreeSet<PathBuf> = krate.tests.iter().filter_map(|f| tested_module(&f.relative)).collect();
    let mut violations = Vec::new();
    for module in &tested {
        let covered = modules.iter().any(|m| m.starts_with(module) || module.starts_with(m));
        if !covered {
            violations.push(Violation::new(
                krate.name(),
                "test-mirror",
                format!("tests for {} have no matching source module", slash(module)),
            ));
        }
    }
    for module in &modules {
        let shown = slash(module);
        let has_tests = tested.iter().any(|t| t == module || t.starts_with(module));
        if !has_tests && !untested.contains(&shown.as_str()) {
            violations.push(Violation::new(
                krate.name(),
                "test-mirror",
                format!("source module {shown} has no test file"),
            ));
        }
        if has_tests && untested.contains(&shown.as_str()) {
            violations.push(Violation::new(
                krate.name(),
                "test-mirror",
                format!("source module {shown} is listed as untested but has tests"),
            ));
        }
    }
    violations
}

/// Module path of a source file relative to `src/`, without extension; `None`
/// for crate roots and `mod.rs` files.
#[must_use]
pub fn module_path(relative: &Path) -> Option<PathBuf> {
    let name = relative.file_name()?.to_str()?;
    if relative.parent() == Some(Path::new("")) && (name == "lib.rs" || name == "main.rs") {
        return None;
    }
    if name == "mod.rs" {
        return None;
    }
    Some(relative.with_extension(""))
}

/// Module path tested by a file relative to `tests/`; a `main.rs` or `mod.rs`
/// stands for its directory. `None` for helpers.
#[must_use]
pub fn tested_module(relative: &Path) -> Option<PathBuf> {
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => p.to_str(),
            _ => None,
        })
        .collect();
    if parts.iter().any(|p| *p == "support" || *p == "support.rs") {
        return None;
    }
    match parts.as_slice() {
        [file] => Some(PathBuf::from(file.strip_suffix(".rs")?)),
        [.., "mod.rs" | "main.rs"] => relative.parent().map(Path::to_path_buf),
        [.., _] => Some(relative.with_extension("")),
        [] => None,
    }
}
