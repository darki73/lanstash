//! The module import rule: named modules of a crate never name a banned path.

use std::path::{Component, Path};

use crate::violation::Violation;
use crate::workspace::{SourceFile, slash};

/// A path that the sources of one module of one package may not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportBan {
    pub package: &'static str,
    /// Module path under `src/` with `/` separators (`cli`, `plan/keys`); covers
    /// `<module>.rs` and every file under `<module>/`. `*` is the whole crate.
    pub module: &'static str,
    /// Banned path as written in code (`std::fs`, `crate::run`). A match is an
    /// occurrence outside a line comment with no identifier character before it,
    /// nor after it when the path ends in one. A trailing `*` makes it a prefix
    /// (`lanstash_adapter_*` names every adapter crate).
    pub path: &'static str,
}

/// A ban on `path` in module `module` of `package`.
#[must_use]
pub const fn ban(package: &'static str, module: &'static str, path: &'static str) -> ImportBan {
    ImportBan { package, module, path }
}

/// Checks the sources of `package` (relative to `src/`) against every ban naming it.
#[must_use]
pub fn check_module_imports(package: &str, sources: &[SourceFile], bans: &[ImportBan]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for ban in bans.iter().filter(|b| b.package == package) {
        for file in sources.iter().filter(|f| in_module(&f.relative, ban.module)) {
            for (index, line) in file.text.lines().enumerate() {
                if names(code(line), ban.path) {
                    violations.push(Violation::new(
                        package,
                        "module-imports",
                        format!(
                            "src/{}:{} names {}, which module {} may not use",
                            slash(&file.relative),
                            index + 1,
                            ban.path,
                            ban.module
                        ),
                    ));
                }
            }
        }
    }
    violations
}

/// Whether the file at `relative` (under `src/`) belongs to `module`.
#[must_use]
pub fn in_module(relative: &Path, module: &str) -> bool {
    if module == "*" {
        return true;
    }
    let parts: Vec<String> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let module: Vec<&str> = module.split('/').collect();
    if parts.len() > module.len() {
        return parts.iter().zip(&module).all(|(p, m)| p == m);
    }
    let Some((last, parents)) = parts.split_last() else { return false };
    parts.len() == module.len()
        && parents.iter().zip(&module).all(|(p, m)| p == m)
        && last.strip_suffix(".rs") == module.last().copied()
}

fn code(line: &str) -> &str {
    line.find("//").map_or(line, |i| &line[..i])
}

fn names(code: &str, path: &str) -> bool {
    let identifier = |c: char| c.is_alphanumeric() || c == '_';
    let (path, prefix) = path.strip_suffix('*').map_or((path, false), |p| (p, true));
    code.match_indices(path).any(|(start, _)| {
        let before = code[..start].chars().next_back();
        let after = code[start + path.len()..].chars().next();
        let open = prefix || !path.chars().next_back().is_some_and(identifier);
        !before.is_some_and(identifier) && (open || !after.is_some_and(identifier))
    })
}
