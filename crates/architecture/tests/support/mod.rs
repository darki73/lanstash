//! Synthetic crates and workspaces for the rule tests (each test file uses some of them).

#![allow(dead_code)]

use std::path::PathBuf;

use lanstash_architecture::manifest::{Dependency, DependencyKind, Manifest};
use lanstash_architecture::workspace::{Crate, SourceFile, Workspace};

pub fn file(relative: &str, text: &str) -> SourceFile {
    SourceFile { relative: PathBuf::from(relative), text: text.to_owned() }
}

pub fn krate(dir: &str, name: &str, deps: &[(&str, DependencyKind)]) -> Crate {
    Crate {
        dir: dir.to_owned(),
        manifest: Manifest {
            name: Some(name.to_owned()),
            dependencies: deps.iter().map(|(n, k)| Dependency { name: (*n).to_owned(), kind: *k }).collect(),
            workspace_lints: true,
        },
        has_readme: true,
        has_changelog: true,
        has_tests_dir: true,
        ..Crate::default()
    }
}

pub fn workspace(crates: Vec<Crate>) -> Workspace {
    let members: Vec<String> = crates.iter().map(|c| c.dir.clone()).collect();
    Workspace { manifest_dirs: members.clone(), members, crates, ..Workspace::default() }
}
