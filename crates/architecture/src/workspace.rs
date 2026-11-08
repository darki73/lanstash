//! The workspace as the rules see it: its members, their manifests and files.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::manifest::{Manifest, parse_manifest, workspace_members};

/// A file, with its path relative to the directory it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub relative: PathBuf,
    pub text: String,
}

/// One member crate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Crate {
    /// Its directory relative to the workspace root, `/`-separated (`crates/app`).
    pub dir: String,
    pub manifest: Manifest,
    /// `.rs` files under `src/`, relative to it.
    pub sources: Vec<SourceFile>,
    /// `.rs` files under `tests/`, relative to it.
    pub tests: Vec<SourceFile>,
    pub has_readme: bool,
    pub has_changelog: bool,
    pub has_tests_dir: bool,
}

impl Crate {
    /// The package name (the directory when the manifest names none).
    #[must_use]
    pub fn name(&self) -> &str {
        self.manifest.name.as_deref().unwrap_or(&self.dir)
    }
}

/// The workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Workspace {
    /// The members, as listed in the root manifest.
    pub members: Vec<String>,
    pub crates: Vec<Crate>,
    /// Every directory under `crates/` holding a `Cargo.toml`, `/`-separated.
    pub manifest_dirs: Vec<String>,
    /// `Cargo.lock`.
    pub lock: String,
    /// Every `.rs`, `.toml` and `.md` file under `crates/`: (path from the root, text).
    pub texts: Vec<(String, String)>,
}

impl Workspace {
    /// The member named `name`.
    #[must_use]
    pub fn package(&self, name: &str) -> Option<&Crate> {
        self.crates.iter().find(|c| c.name() == name)
    }

    /// Whether `name` is a member package.
    #[must_use]
    pub fn is_member(&self, name: &str) -> bool {
        self.package(name).is_some()
    }
}

/// `path` with `/` separators.
#[must_use]
pub fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn walk(dir: &Path, base: &Path, keep: &dyn Fn(&Path) -> bool, out: &mut Vec<SourceFile>) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>()?;
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "node_modules" || n == "target") {
                continue;
            }
            walk(&p, base, keep, out)?;
        } else if keep(&p) {
            let relative = p.strip_prefix(base).unwrap_or(&p).to_path_buf();
            out.push(SourceFile { relative, text: fs::read_to_string(&p)? });
        }
    }
    Ok(())
}

/// The files below `dir` that `keep` accepts, relative to `dir`; none when `dir` is missing.
///
/// # Errors
/// A directory or file could not be read.
pub fn files(dir: &Path, keep: &dyn Fn(&Path) -> bool) -> io::Result<Vec<SourceFile>> {
    let mut out = Vec::new();
    if dir.is_dir() {
        walk(dir, dir, keep, &mut out)?;
    }
    Ok(out)
}

fn is_rust(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "rs")
}

/// Loads one crate from `root/dir`.
///
/// # Errors
/// Its files could not be read.
pub fn load_crate(root: &Path, dir: &str) -> io::Result<Crate> {
    let path = root.join(dir);
    let manifest = parse_manifest(&fs::read_to_string(path.join("Cargo.toml"))?);
    Ok(Crate {
        dir: dir.to_owned(),
        manifest,
        sources: files(&path.join("src"), &is_rust)?,
        tests: files(&path.join("tests"), &is_rust)?,
        has_readme: path.join("README.md").is_file(),
        has_changelog: path.join("CHANGELOG.md").is_file(),
        has_tests_dir: path.join("tests").is_dir(),
    })
}

/// Loads the workspace at `root`.
///
/// # Errors
/// A manifest or source could not be read.
pub fn load(root: &Path) -> io::Result<Workspace> {
    let members = workspace_members(&fs::read_to_string(root.join("Cargo.toml"))?);
    let crates = members.iter().map(|m| load_crate(root, m)).collect::<io::Result<Vec<_>>>()?;
    let crates_dir = root.join("crates");
    let manifests = files(&crates_dir, &|p: &Path| p.file_name().is_some_and(|n| n == "Cargo.toml"))?;
    let manifest_dirs =
        manifests.iter().map(|f| format!("crates/{}", slash(f.relative.parent().unwrap_or(Path::new(""))))).collect();
    let texts = files(&crates_dir, &|p: &Path| p.extension().is_some_and(|e| e == "rs" || e == "toml" || e == "md"))?
        .into_iter()
        .map(|f| (format!("crates/{}", slash(&f.relative)), f.text))
        .collect();
    let lock = fs::read_to_string(root.join("Cargo.lock")).unwrap_or_default();
    Ok(Workspace { members, crates, manifest_dirs, lock, texts })
}
