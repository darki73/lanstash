mod support;

use std::path::{Path, PathBuf};

use lanstash_architecture::mirror::{check_test_mirror, module_path, tested_module};
use support::{file, krate};

#[test]
fn maps_sources_to_modules() {
    assert_eq!(module_path(Path::new("lib.rs")), None);
    assert_eq!(module_path(Path::new("main.rs")), None);
    assert_eq!(module_path(Path::new("segment/mod.rs")), None);
    assert_eq!(module_path(Path::new("segment/footer.rs")), Some(PathBuf::from("segment/footer")));
    assert_eq!(module_path(Path::new("plan/main.rs")), Some(PathBuf::from("plan/main")));
}

#[test]
fn maps_tests_to_modules() {
    assert_eq!(tested_module(Path::new("serve.rs")), Some(PathBuf::from("serve")));
    assert_eq!(tested_module(Path::new("segment/main.rs")), Some(PathBuf::from("segment")));
    assert_eq!(tested_module(Path::new("segment/footer.rs")), Some(PathBuf::from("segment/footer")));
    assert_eq!(tested_module(Path::new("support/mod.rs")), None);
    assert_eq!(tested_module(Path::new("fetch/support.rs")), None);
}

#[test]
fn every_module_has_tests_and_every_test_a_module() {
    let mut c = krate("crates/x", "x", &[]);
    c.sources = ["lib.rs", "a.rs", "b.rs", "seg/mod.rs", "seg/head.rs", "seg/foot.rs", "quiet.rs"]
        .iter()
        .map(|p| file(p, ""))
        .collect();
    c.tests = ["a.rs", "b/main.rs", "b/split.rs", "seg/main.rs", "seg/head.rs", "gone.rs", "support/mod.rs"]
        .iter()
        .map(|p| file(p, ""))
        .collect();
    let v: Vec<String> = check_test_mirror(&c, &["quiet"]).into_iter().map(|v| v.detail).collect();
    assert_eq!(v, vec!["tests for gone have no matching source module", "source module seg/foot has no test file"]);
    c.tests.push(file("quiet.rs", ""));
    c.tests.push(file("seg/foot.rs", ""));
    c.tests.retain(|f| f.relative != Path::new("gone.rs"));
    let v: Vec<String> = check_test_mirror(&c, &["quiet"]).into_iter().map(|v| v.detail).collect();
    assert_eq!(v, vec!["source module quiet is listed as untested but has tests"]);
}
