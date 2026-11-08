mod support;

use lanstash_architecture::policy::{CratePolicy, Layer, Policy};
use lanstash_architecture::sources::check_sources;
use support::{file, krate};

const fn row(layer: Layer, no_std: bool, no_io: bool) -> CratePolicy {
    CratePolicy { dir: "crates/x", package: "x", layer, internal: &[], dev_internal: &[], no_std, no_io }
}

const POLICY: Policy = Policy {
    crates: &[],
    external: &[],
    direct_external: &[],
    max_lines: 400,
    oversized: &[],
    reexports: &["crates/x/src/old.rs"],
    undocumented: &[],
    untested: &[],
    module_bans: &[],
    unsafe_code: &[],
};

fn rules(sources: &[(&str, &str)], row: &CratePolicy) -> Vec<&'static str> {
    let mut c = krate("crates/x", "x", &[]);
    c.sources = sources.iter().map(|(p, t)| file(p, t)).collect();
    check_sources(&c, row, &POLICY).into_iter().map(|v| v.rule).collect()
}

const ROOT: &str = "#![forbid(unsafe_code)]\n";

#[test]
fn crate_roots_forbid_unsafe_and_keep_no_std() {
    let core = row(Layer::Core, true, false);
    assert!(rules(&[("lib.rs", "#![no_std]\n#![forbid(unsafe_code)]\n")], &core).is_empty());
    assert_eq!(rules(&[("lib.rs", "#![no_std]\n")], &core), vec!["forbid-unsafe"]);
    assert_eq!(rules(&[("lib.rs", ROOT)], &core), vec!["no-std"]);
    assert_eq!(
        rules(&[("main.rs", "// #![forbid(unsafe_code)]\n")], &row(Layer::Root, false, false)),
        vec!["forbid-unsafe"]
    );
    assert!(rules(&[("a.rs", "fn f() {}\n")], &core).is_empty(), "only roots carry the attributes");
}

#[test]
fn unsafe_only_in_the_files_the_policy_names() {
    const ALLOWING: Policy = Policy { unsafe_code: &[("x", "crates/x/src/sys.rs")], ..POLICY };
    let r = row(Layer::Adapter, false, false);
    let check = |policy: &Policy, sources: &[(&str, &str)]| {
        let mut c = krate("crates/x", "x", &[]);
        c.sources = sources.iter().map(|(p, t)| file(p, t)).collect();
        check_sources(&c, &r, policy).into_iter().map(|v| v.rule).collect::<Vec<_>>()
    };
    let call = "#[allow(unsafe_code)]\nfn f() { let n = unsafe { g() }; }\n";
    assert!(check(&ALLOWING, &[("lib.rs", "#![deny(unsafe_code)]\n"), ("sys.rs", call)]).is_empty());
    assert_eq!(
        check(&ALLOWING, &[("lib.rs", "#![deny(unsafe_code)]\n"), ("other.rs", call)]),
        vec!["unsafe-where-allowed"; 2],
        "the lift and the block, outside the named file"
    );
    assert_eq!(check(&ALLOWING, &[("lib.rs", ROOT)]), vec!["forbid-unsafe"], "the allowed crate denies, not forbids");
    assert_eq!(check(&POLICY, &[("lib.rs", ROOT), ("a.rs", "unsafe fn f() {}\n")]), vec!["unsafe-where-allowed"]);
    let words = "fn unsafe_ish() {}\n// unsafe in a comment\nlet s = \"unsafe\";\n";
    assert!(
        check(&POLICY, &[("lib.rs", ROOT), ("a.rs", words)]).is_empty(),
        "names, comments and strings are not the keyword"
    );
}

#[test]
fn no_reexports_unless_listed() {
    let r = row(Layer::Core, false, false);
    assert_eq!(
        rules(&[("a.rs", "pub use crate::b::C;\npub(crate) use x::Y;\n")], &r),
        vec!["no-reexport", "no-reexport"]
    );
    assert!(
        rules(&[("old.rs", "pub use crate::b::C;\n"), ("b.rs", "use crate::b::C;\n// pub use x;\n")], &r).is_empty()
    );
}

#[test]
fn no_inline_tests_and_no_global_state() {
    let r = row(Layer::Core, false, false);
    assert_eq!(rules(&[("a.rs", "#[cfg(test)]\nmod t {\n#[test]\nfn x() {}\n}\n")], &r), vec!["no-inline-tests"; 2]);
    assert_eq!(rules(&[("a.rs", "static S: OnceLock<u8> = OnceLock::new();\n")], &r), vec!["no-global-state"]);
    assert!(rules(&[("a.rs", "// static mut X: u8 = 0;\n")], &r).is_empty());
}

#[test]
fn only_the_root_reads_arguments_environment_and_clocks() {
    let src = [("a.rs", "let t = Instant::now();\nlet a = std::env::args();\n")];
    assert_eq!(rules(&src, &row(Layer::Adapter, false, false)), vec!["root-only", "root-only"]);
    assert!(rules(&src, &row(Layer::Root, false, false)).is_empty());
    assert!(rules(&src, &row(Layer::Tool, false, false)).is_empty());
}

#[test]
fn crates_without_io_name_no_io_module() {
    let src = [("a.rs", "use std::sync::Arc;\nuse std::{fs, thread};\n")];
    assert_eq!(rules(&src, &row(Layer::Core, false, true)), vec!["no-io"]);
    assert!(rules(&src, &row(Layer::Adapter, false, false)).is_empty());
}
