//! Small, dependency-free parsers for `Cargo.toml` and `Cargo.lock`: just
//! enough to check the workspace's own files.

use std::collections::BTreeSet;

/// Kind of a declared dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyKind {
    Normal,
    Dev,
    Build,
}

/// A dependency declared in a manifest, by the name on the left-hand side.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Dependency {
    pub name: String,
    pub kind: DependencyKind,
}

/// What the rules read from one `Cargo.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// `[package] name`.
    pub name: Option<String>,
    /// Normal, dev, build and target-specific dependencies.
    pub dependencies: Vec<Dependency>,
    /// `[lints]` starts with `workspace = true`.
    pub workspace_lints: bool,
}

/// The kind of a dependency table header (`dependencies`, `dev-dependencies`,
/// `target.'cfg(unix)'.build-dependencies`), if it is one.
fn table_kind(section: &str) -> Option<DependencyKind> {
    let last = section.rsplit('.').next().unwrap_or(section);
    match last {
        "dependencies" => Some(DependencyKind::Normal),
        "dev-dependencies" => Some(DependencyKind::Dev),
        "build-dependencies" => Some(DependencyKind::Build),
        _ => None,
    }
}

/// The kind and name of a `[dependencies.foo]` style header, if it is one.
fn dotted(section: &str) -> Option<(DependencyKind, String)> {
    for (table, kind) in [
        ("build-dependencies.", DependencyKind::Build),
        ("dev-dependencies.", DependencyKind::Dev),
        ("dependencies.", DependencyKind::Normal),
    ] {
        if let Some(i) = section.find(table) {
            let before = section[..i].chars().next_back();
            if before.is_none_or(|c| c == '.') {
                let name = section[i + table.len()..].trim_matches('"').to_owned();
                return Some((kind, name));
            }
        }
    }
    None
}

/// Parses a member manifest.
#[must_use]
pub fn parse_manifest(toml: &str) -> Manifest {
    let mut m = Manifest::default();
    let mut section = String::new();
    let mut in_deps: Option<DependencyKind> = None;
    let mut lints_first = false;
    for raw in toml.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s.trim().to_owned();
            in_deps = table_kind(&section);
            if in_deps.is_none()
                && let Some((kind, name)) = dotted(&section)
            {
                m.dependencies.push(Dependency { name, kind });
            }
            lints_first = section == "lints";
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim().trim_matches('"'), value.trim());
        if lints_first {
            m.workspace_lints = key == "workspace" && value == "true";
            lints_first = false;
        }
        if section == "package" && key == "name" {
            m.name = Some(value.trim_matches('"').to_owned());
        }
        if let Some(kind) = in_deps {
            let name = key.split('.').next().unwrap_or(key).to_owned();
            m.dependencies.push(Dependency { name, kind });
        }
    }
    m.dependencies.sort();
    m.dependencies.dedup();
    m
}

/// The `[workspace] members` of the root manifest, as written.
#[must_use]
pub fn workspace_members(toml: &str) -> Vec<String> {
    let Some(start) = toml.find("members") else { return Vec::new() };
    let rest = &toml[start..];
    let (Some(open), Some(close)) = (rest.find('['), rest.find(']')) else { return Vec::new() };
    rest[open + 1..close]
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .flat_map(|l| l.split(','))
        .map(|s| s.trim().trim_matches('"').to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `name = "..."` of every `[[package]]` in a `Cargo.lock`.
#[must_use]
pub fn lock_packages(lock: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut in_pkg = false;
    for line in lock.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_pkg = line == "[[package]]";
            continue;
        }
        if in_pkg && let Some(v) = line.strip_prefix("name = ") {
            out.insert(v.trim_matches('"').to_owned());
        }
    }
    out
}
