//! Crate file rules: `README.md`, `CHANGELOG.md` and `tests/` per crate, and the workspace lints.

use crate::violation::Violation;
use crate::workspace::Crate;

/// `README.md`, `CHANGELOG.md` (unless `undocumented` names the crate) and `tests/`.
#[must_use]
pub fn check_crate_files(krate: &Crate, undocumented: &[&str]) -> Vec<Violation> {
    let pending = undocumented.contains(&krate.name());
    [
        (krate.has_readme || pending, "README.md"),
        (krate.has_changelog || pending, "CHANGELOG.md"),
        (krate.has_tests_dir, "tests/"),
    ]
    .into_iter()
    .filter(|(present, _)| !present)
    .map(|(_, file)| Violation::new(krate.name(), "crate-files", format!("missing {file}")))
    .collect()
}

/// `[lints] workspace = true`, but for a crate the policy lets hold `unsafe` (`own_lints`): the
/// workspace's forbid it, so it keeps its own (its root denies `unsafe`, checked with the sources).
#[must_use]
pub fn check_lints(krate: &Crate, own_lints: bool) -> Vec<Violation> {
    if krate.manifest.workspace_lints || own_lints {
        Vec::new()
    } else {
        vec![Violation::new(krate.name(), "workspace-lints", "must use `[lints] workspace = true`")]
    }
}
