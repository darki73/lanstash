use lanstash_architecture::manifest::{Dependency, DependencyKind, lock_packages, parse_manifest, workspace_members};

fn dep(name: &str, kind: DependencyKind) -> Dependency {
    Dependency { name: name.to_owned(), kind }
}

#[test]
fn parses_dependencies_of_every_kind() {
    let toml = "[package]\nname = \"x\"\nversion.workspace = true\n[dependencies]\na = { path = \"../a\" }\nb = \"1\" # c\n\
                [dev-dependencies]\nd = \"1\"\n[target.'cfg(unix)'.dependencies]\ne = \"1\"\n\
                [dependencies.f]\nversion = \"1\"\n[build-dependencies]\ng.workspace = true\n[features]\nt = []\n\
                [lints]\nworkspace = true\n";
    let m = parse_manifest(toml);
    assert_eq!(m.name.as_deref(), Some("x"));
    use DependencyKind::{Build, Dev, Normal};
    assert_eq!(
        m.dependencies,
        vec![dep("a", Normal), dep("b", Normal), dep("d", Dev), dep("e", Normal), dep("f", Normal), dep("g", Build)]
    );
    assert!(m.workspace_lints);
}

#[test]
fn lints_must_come_from_the_workspace() {
    assert!(!parse_manifest("[package]\nname = \"x\"\n").workspace_lints);
    assert!(!parse_manifest("[lints.rust]\nunsafe_code = \"forbid\"\n").workspace_lints);
    assert!(!parse_manifest("[lints]\nworkspace = false\n").workspace_lints);
}

#[test]
fn reads_the_member_list() {
    let toml = "[workspace]\nresolver = \"3\"\nmembers = [\n  \"crates/a\", # one\n  \"crates/b/c\",\n]\n";
    assert_eq!(workspace_members(toml), vec!["crates/a", "crates/b/c"]);
    assert_eq!(workspace_members("members = [\"x\", \"y\"]"), vec!["x", "y"]);
    assert!(workspace_members("[package]\n").is_empty());
}

#[test]
fn parses_lock_packages() {
    let lock = "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"0.1.0\"\ndependencies = [\n \"b\",\n]\n\n\
                [[package]]\nname = \"b\"\nversion = \"1.0.0\"\n";
    assert_eq!(lock_packages(lock), ["a", "b"].iter().map(|s| (*s).to_owned()).collect());
}
