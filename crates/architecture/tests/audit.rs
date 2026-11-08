//! The lanstash workspace keeps every rule of `policy::LANSTASH`. A failure
//! here lists each broken rule.

use std::path::Path;

use lanstash_architecture::audit::audit;
use lanstash_architecture::policy::LANSTASH;
use lanstash_architecture::workspace::{Workspace, load};

fn repository() -> Workspace {
    load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")).expect("workspace readable")
}

#[test]
fn the_workspace_keeps_every_rule() {
    let violations = audit(&repository(), &LANSTASH);
    let shown: Vec<String> = violations.iter().map(ToString::to_string).collect();
    assert!(shown.is_empty(), "{} violations:\n{}", shown.len(), shown.join("\n"));
}

#[test]
fn every_member_is_loaded() {
    let ws = repository();
    assert_eq!(ws.crates.len(), ws.members.len());
    assert!(ws.package("lanstash-domain").is_some());
    assert!(ws.package("lanstash").is_some_and(|c| c.sources.iter().any(|f| f.relative == Path::new("main.rs"))));
}

#[test]
fn the_allow_lists_name_existing_things() {
    let ws = repository();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    for path in LANSTASH.oversized.iter().chain(LANSTASH.reexports) {
        assert!(root.join(path).is_file(), "{path} is listed but does not exist");
    }
    for package in LANSTASH.undocumented {
        assert!(ws.is_member(package), "{package} is listed but is not a member");
    }
    for (package, module, why) in LANSTASH.untested {
        assert!(!why.is_empty(), "{package}/{module} needs a reason");
        let c = ws.package(package).unwrap_or_else(|| panic!("{package} is not a member"));
        let (file, dir) = (format!("{module}.rs"), format!("{module}/mod.rs"));
        assert!(
            c.sources.iter().any(|f| {
                let p = f.relative.to_string_lossy().replace('\\', "/");
                p == file || p == dir
            }),
            "{package}: no {file} or {dir}"
        );
    }
}
