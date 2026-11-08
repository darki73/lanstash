mod support;

use std::path::Path;

use lanstash_architecture::module_imports::{ban, check_module_imports, in_module};
use support::file;

#[test]
fn modules_cover_their_file_and_directory() {
    assert!(in_module(Path::new("plan.rs"), "plan"));
    assert!(in_module(Path::new("plan/keys.rs"), "plan"));
    assert!(in_module(Path::new("plan/keys.rs"), "plan/keys"));
    assert!(!in_module(Path::new("planner.rs"), "plan"));
    assert!(!in_module(Path::new("plan.rs"), "plan/keys"));
    assert!(in_module(Path::new("anything/at/all.rs"), "*"));
}

#[test]
fn reports_banned_paths_outside_comments() {
    let bans = [ban("x", "domain", "std::fs"), ban("x", "domain", "crate::app"), ban("y", "*", "std::net")];
    let sources = [
        file("domain.rs", "use std::fs::File;\n// std::fs in a comment\nuse crate::application::X;\n"),
        file("domain/sub.rs", "use crate::app::Y;\nuse my_std::fs;\n"),
        file("app.rs", "use std::fs;\n"),
    ];
    let v: Vec<String> = check_module_imports("x", &sources, &bans).into_iter().map(|v| v.detail).collect();
    assert_eq!(
        v,
        vec![
            "src/domain.rs:1 names std::fs, which module domain may not use",
            "src/domain/sub.rs:1 names crate::app, which module domain may not use",
        ]
    );
    assert!(check_module_imports("z", &sources, &bans).is_empty());
}

#[test]
fn a_trailing_star_bans_a_prefix() {
    let bans = [ban("x", "*", "lanstash_adapter_*")];
    let sources = [
        file("cli.rs", "use lanstash_adapter_fs::dir_fs;\n"),
        file("config.rs", "use lanstash_adapters_old::x;\nuse my_lanstash_adapter_x::y;\n"),
    ];
    let v: Vec<String> = check_module_imports("x", &sources, &bans).into_iter().map(|v| v.detail).collect();
    assert_eq!(v, vec!["src/cli.rs:1 names lanstash_adapter_*, which module * may not use"]);
}
