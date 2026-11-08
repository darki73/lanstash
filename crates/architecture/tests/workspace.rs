use std::fs;
use std::path::{Path, PathBuf};

use lanstash_architecture::workspace::{files, load, slash};

/// A scratch workspace under the target directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lanstash-architecture-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, rel: &str, text: &str) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn loads_members_their_files_and_stray_manifests() {
    let s = Scratch::new("load");
    s.write("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n");
    s.write("Cargo.lock", "[[package]]\nname = \"a\"\n");
    s.write("crates/a/Cargo.toml", "[package]\nname = \"a\"\n[lints]\nworkspace = true\n");
    s.write("crates/a/README.md", "# a\n");
    s.write("crates/a/src/lib.rs", "pub mod b;\n");
    s.write("crates/a/src/b/c.rs", "\n");
    s.write("crates/a/tests/b.rs", "\n");
    s.write("crates/a/src/page.html", "<p>\n");
    s.write("crates/stray/Cargo.toml", "[package]\nname = \"stray\"\n");
    let ws = load(&s.0).unwrap();
    assert_eq!(ws.members, vec!["crates/a"]);
    let a = ws.package("a").unwrap();
    assert_eq!(a.dir, "crates/a");
    let sources: Vec<String> = a.sources.iter().map(|f| slash(&f.relative)).collect();
    assert_eq!(sources, vec!["b/c.rs", "lib.rs"]);
    assert!(a.has_readme && !a.has_changelog && a.has_tests_dir);
    assert!(a.manifest.workspace_lints);
    assert_eq!(ws.manifest_dirs, vec!["crates/a", "crates/stray"]);
    let texts: Vec<&str> = ws.texts.iter().map(|(p, _)| p.as_str()).collect();
    assert!(texts.contains(&"crates/a/README.md") && !texts.contains(&"crates/a/src/page.html"), "{texts:?}");
    assert!(ws.is_member("a") && !ws.is_member("stray"));
}

#[test]
fn a_missing_directory_has_no_files() {
    assert!(files(Path::new("does/not/exist"), &|_| true).unwrap().is_empty());
}
