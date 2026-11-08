//! Tests for `lanstash_domain::accounts`: names, passwords, roles and the kept text.

use lanstash_domain::accounts::{
    Account, Permission, Role, encode, hex, is_link, name_problem, parse, password_problem, unhex,
};

#[test]
fn names_and_passwords_are_checked() {
    for good in ["admin", "player.one", "a_b-c9", &"x".repeat(32)] {
        assert_eq!(name_problem(good), None, "{good}");
    }
    for bad in ["", "Admin", "a b", "ä", "a/b", &"x".repeat(33)] {
        assert!(name_problem(bad).is_some(), "{bad}");
    }
    assert!(password_problem("short").is_some());
    assert_eq!(password_problem("ten chars!"), None);
    assert_eq!(password_problem("ünïcödé-ok"), None, "characters, not bytes");
    assert!(password_problem(&"p".repeat(129)).is_some());
}

#[test]
fn accounts_round_trip_and_bad_lines_are_skipped() {
    let accounts = [
        Account { name: "boss".into(), role: Role::Admin, hash: "pbkdf2-sha256$1$aa$bb".into(), links: vec![] },
        Account {
            name: "guest".into(),
            role: Role::Player,
            hash: "x".into(),
            links: vec!["steam/someone".into(), "epicgames/other".into()],
        },
    ];
    let text = encode(&accounts);
    assert_eq!(text, "boss admin pbkdf2-sha256$1$aa$bb\nguest player x steam/someone,epicgames/other\n");
    assert!(is_link("steam/someone") && !is_link("steam") && !is_link("Steam/x") && !is_link("steam/a b"));
    assert!(parse("x viewer h steam/a,bad\n").is_empty(), "a line with a bad link is no account");
    assert_eq!(parse(&text), accounts);
    let messy = "boss admin h\nBad admin h\nnorole h\nx owner h\ny viewer h extra\n\nz viewer h2\n";
    assert_eq!(parse(messy).iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["boss", "z"]);
}

#[test]
fn roles_have_names_and_admin_is_above_viewer() {
    assert_eq!([Role::Admin.name(), Role::Viewer.name()], ["admin", "viewer"]);
    assert_eq!(Role::parse("admin"), Some(Role::Admin));
    assert_eq!(Role::parse("root"), None);
    assert!(Role::Admin > Role::Viewer);
}

#[test]
fn a_role_is_a_set_of_permissions() {
    assert!(Permission::ALL.iter().all(|p| Role::Admin.can(*p)));
    assert!(Permission::ALL.iter().all(|p| !Role::Viewer.can(*p)));
    assert_eq!(Role::Player.permissions(), [Permission::PrefillOwn]);
    assert_eq!(Role::parse("player"), Some(Role::Player));
    assert_eq!(Permission::PrefillOwn.name(), "prefill.own");
    let names: std::collections::BTreeSet<_> = Permission::ALL.iter().map(|p| p.name()).collect();
    assert_eq!(names.len(), Permission::ALL.len(), "every name its own");
}

#[test]
fn hex_round_trips() {
    let bytes = [0u8, 1, 0xab, 0xff];
    assert_eq!(hex(&bytes), "0001abff");
    assert_eq!(unhex("0001ABff"), Some(bytes.to_vec()));
    assert_eq!(unhex("abc"), None);
    assert_eq!(unhex("zz"), None);
}
