mod support;

use lanstash_architecture::deps::{
    check_acyclic, check_internal_dependencies, check_layers, check_lock, check_policy_coverage,
};
use lanstash_architecture::manifest::DependencyKind::{Dev, Normal};
use lanstash_architecture::policy::{CratePolicy, Layer, Policy};
use support::{krate, workspace};

const fn row(dir: &'static str, package: &'static str, layer: Layer, internal: &'static [&'static str]) -> CratePolicy {
    CratePolicy { dir, package, layer, internal, dev_internal: &[], no_std: false, no_io: false }
}

const ROWS: &[CratePolicy] = &[
    row("crates/d", "d", Layer::Domain, &[]),
    row("crates/a", "a", Layer::Core, &["d"]),
    row("crates/p", "p", Layer::Platform("p"), &["d", "a"]),
    row("crates/r", "r", Layer::Root, &["d", "a", "p"]),
];

fn policy(crates: &'static [CratePolicy]) -> Policy {
    Policy {
        crates,
        external: &[],
        direct_external: &[],
        max_lines: 400,
        oversized: &[],
        reexports: &[],
        undocumented: &[],
        untested: &[],
        module_bans: &[],
        unsafe_code: &[],
    }
}

#[test]
fn every_crate_needs_a_row_at_its_directory() {
    let mut ws =
        workspace(vec![krate("crates/d", "d", &[]), krate("crates/x", "x", &[]), krate("crates/elsewhere", "a", &[])]);
    ws.manifest_dirs.push("crates/stray".into());
    let v = check_policy_coverage(&ws, &policy(ROWS));
    let what: Vec<(&str, &str)> = v.iter().map(|v| (v.package.as_str(), v.detail.as_str())).collect();
    assert!(what.contains(&("x", "crate has no policy entry")), "{what:?}");
    assert!(what.contains(&("a", "crate is at crates/elsewhere, its policy says crates/a")), "{what:?}");
    assert!(what.contains(&("crates/stray", "manifest is not a workspace member")), "{what:?}");
    assert!(what.contains(&("p", "policy entry is not a workspace member")), "{what:?}");
}

#[test]
fn only_allowed_internal_dependencies() {
    let ws = workspace(vec![
        krate("crates/d", "d", &[("a", Normal)]),
        krate("crates/a", "a", &[("d", Normal), ("serde", Normal)]),
        krate("crates/p", "p", &[("d", Dev)]),
    ]);
    let v = check_internal_dependencies(&ws, &policy(ROWS));
    let rules: Vec<(&str, &str)> = v.iter().map(|v| (v.package.as_str(), v.rule)).collect();
    assert_eq!(rules, vec![("d", "internal-dependency"), ("a", "external-dependency")]);
}

#[test]
fn a_third_party_crate_only_where_it_is_approved() {
    let ws = workspace(vec![
        krate("crates/d", "d", &[]),
        krate("crates/a", "a", &[("d", Normal), ("rustls", Normal)]),
        krate("crates/p", "p", &[("rustls", Normal)]),
    ]);
    let approved = Policy { external: &["rustls"], direct_external: &[("a", &["rustls"])], ..policy(ROWS) };
    let v = check_internal_dependencies(&ws, &approved);
    let rules: Vec<(&str, &str)> = v.iter().map(|v| (v.package.as_str(), v.rule)).collect();
    assert_eq!(rules, vec![("p", "external-dependency")], "approved for a only");
}
#[test]
fn platforms_build_only_on_core_and_core_never_names_a_platform() {
    assert!(check_layers(&policy(ROWS)).is_empty());
    const BAD: &[CratePolicy] = &[
        row("crates/d", "d", Layer::Domain, &[]),
        row("crates/a", "a", Layer::Core, &["p"]),
        row("crates/b", "b", Layer::Core, &["ad"]),
        row("crates/ad", "ad", Layer::Adapter, &["p"]),
        row("crates/p", "p", Layer::Platform("p"), &["q", "ad"]),
        row("crates/q", "q", Layer::Platform("q"), &[]),
        row("crates/q2", "q2", Layer::Platform("q"), &["q"]),
    ];
    let v = check_layers(&policy(BAD));
    let who: Vec<&str> = v.iter().map(|v| v.package.as_str()).collect();
    assert_eq!(who, vec!["a", "b", "ad", "p", "p"], "{v:?}");
}

#[test]
fn cycles_are_found_but_a_self_dev_dependency_is_not_one() {
    let ws = workspace(vec![
        krate("crates/a", "a", &[("b", Normal), ("a", Dev)]),
        krate("crates/b", "b", &[("c", Normal)]),
        krate("crates/c", "c", &[]),
    ]);
    assert!(check_acyclic(&ws).is_empty());
    let ws = workspace(vec![krate("crates/a", "a", &[("b", Normal)]), krate("crates/b", "b", &[("a", Dev)])]);
    let v = check_acyclic(&ws);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].detail, "a -> b -> a");
}

#[test]
fn the_lock_file_holds_only_members() {
    let mut ws = workspace(vec![krate("crates/a", "a", &[])]);
    ws.lock = "[[package]]\nname = \"a\"\n\n[[package]]\nname = \"libc\"\n".into();
    let v = check_lock(&ws, &policy(ROWS));
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].package, "libc");
}
