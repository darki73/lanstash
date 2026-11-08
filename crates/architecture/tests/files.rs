mod support;

use lanstash_architecture::files::{check_crate_files, check_lints};
use support::krate;

#[test]
fn every_crate_has_readme_changelog_and_tests() {
    let mut c = krate("crates/x", "x", &[]);
    assert!(check_crate_files(&c, &[]).is_empty());
    c.has_readme = false;
    c.has_changelog = false;
    c.has_tests_dir = false;
    let missing: Vec<String> = check_crate_files(&c, &[]).into_iter().map(|v| v.detail).collect();
    assert_eq!(missing, vec!["missing README.md", "missing CHANGELOG.md", "missing tests/"]);
    let pending: Vec<String> = check_crate_files(&c, &["x"]).into_iter().map(|v| v.detail).collect();
    assert_eq!(pending, vec!["missing tests/"], "listed crates may lack the documents, never tests/");
}

#[test]
fn every_crate_uses_the_workspace_lints() {
    let mut c = krate("crates/x", "x", &[]);
    assert!(check_lints(&c, false).is_empty());
    c.manifest.workspace_lints = false;
    assert_eq!(check_lints(&c, false).len(), 1);
    assert!(check_lints(&c, true).is_empty(), "the crate allowed `unsafe` keeps lints of its own");
}
