//! Tests for `lanstash_steam_app::library`, against a scripted account.

mod support;

use std::sync::Arc;

use lanstash_ports::clock::FixedClock;
use lanstash_ports::names::GameNames;
use lanstash_steam_app::library::{SteamLibrary, read};
use support::account::services;

#[test]
fn the_library_holds_the_licensed_apps_and_their_depots() {
    let (s, _) = services(true);
    let (mut c, _) = s.session("someone").unwrap();
    let lib = read(&mut c).unwrap();
    assert_eq!(lib.apps.len(), 1);
    let app = lib.app(100).unwrap();
    assert_eq!((app.title().as_str(), app.build), ("Some Game", Some(555)));
    assert_eq!(lib.depot_app.get(&101), Some(&100));
    assert_eq!(lib.depot_app.get(&228_989), Some(&228_980), "a shared depot is its provider's");
    assert!(lib.app(7).is_none());
    assert_eq!(lib.owned_depots.iter().copied().collect::<Vec<_>>(), [101, 102, 103], "the depots the license grants");
}

#[test]
fn the_kept_library_names_depots_after_their_app() {
    let (s, _) = services(true);
    let lib = SteamLibrary::new(s, Arc::new(FixedClock(1_000)));
    assert!(lib.get().is_none() && lib.name("steam", "101").is_none());
    assert_eq!(lib.refresh().unwrap().at_ms, 1_000);
    assert_eq!(lib.name("steam", "101").as_deref(), Some("Some Game"));
    assert_eq!(lib.name("steam", "100").as_deref(), Some("Some Game"), "the app itself");
    assert_eq!(lib.group("steam", "102").as_deref(), Some("100"));
    assert!(lib.name("blizzard", "101").is_none() && lib.name("steam", "999").is_none());
    assert!(lib.error().is_none());
}

#[test]
fn steams_answers_give_a_library() {
    use lanstash_steam_app::library::from_raw;
    use lanstash_steam_app::raw::Raw;
    let raw = Raw {
        owned: vec![101, 201],
        apps: vec![
            (100, br#""appinfo" { "common" { "name" "Some Game" "type" "Game" } }"#.to_vec()),
            (7, b"not key-values".to_vec()),
        ],
    };
    let lib = from_raw(&raw);
    assert_eq!(lib.app(100).unwrap().title(), "Some Game");
    assert_eq!(lib.app(7).unwrap().title(), "7", "info without a name: named by its id, as when read from Steam");
    assert_eq!(lib.owned_depots.iter().copied().collect::<Vec<_>>(), [101, 201]);
}

#[test]
fn without_a_sign_in_the_failure_is_kept() {
    let (s, _) = services(false);
    let lib = SteamLibrary::new(s, Arc::new(FixedClock(0)));
    assert!(lib.refresh().is_err());
    assert_eq!(lib.error().as_deref(), Some("not signed in to Steam: sign in from the web UI"));
}

#[test]
fn an_install_takes_the_shared_depots_whose_provider_the_library_knows() {
    use lanstash_steam_app::library::Library;
    use lanstash_steam_domain::apps::{Target, app};
    use lanstash_steam_domain::kv::parse_text;
    let info = |id: u32, text: &str| app(id, &parse_text(text.as_bytes()).unwrap());
    let game = info(
        100,
        r#""appinfo" { "depots" {
            "101" { "manifests" { "public" { "gid" "11" "size" "9" "download" "3" } } }
            "228989" { "config" { "oslist" "windows" } "depotfromapp" "228980" }
            "777" { "depotfromapp" "999" }
            "102" { "dlcappid" "500" "manifests" { "public" { "gid" "12" "size" "1" "download" "1" } } }
            "103" { "dlcappid" "600" "manifests" { "public" { "gid" "13" "size" "1" "download" "1" } } }
        } }"#,
    );
    let redist = info(
        228_980,
        r#""appinfo" { "depots" { "228989" { "manifests" { "public" { "gid" "42" "size" "5" "download" "2" } } } } }"#,
    );
    let owned_dlc = info(500, r#""appinfo" { "common" { "type" "DLC" } }"#);
    let owned_depots = [101, 102, 103, 228_989].into_iter().collect();
    let lib = Library { apps: vec![game.clone(), owned_dlc, redist], owned_depots, ..Library::default() };
    let got: Vec<(u32, u32, Option<u64>)> =
        lib.install(&game, &Target::default()).iter().map(|(owner, d)| (*owner, d.id, d.manifest)).collect();
    assert_eq!(
        got,
        [(100, 101, Some(11)), (100, 102, Some(12)), (228_980, 228_989, Some(42))],
        "DLC 500 is owned (600 is not); depot 777's provider is unknown"
    );
}

#[test]
fn a_dlc_with_depots_of_its_own_is_part_of_its_games_install_and_name() {
    use std::collections::BTreeSet;

    use lanstash_steam_app::library::Library;
    use lanstash_steam_domain::apps::{Target, app};
    use lanstash_steam_domain::kv::parse_text;
    let info = |id: u32, text: &str| app(id, &parse_text(text.as_bytes()).unwrap());
    // Shaped like Cyberpunk 2077 (1091500) and Phantom Liberty (2138330).
    let game = info(
        100,
        r#""appinfo" { "common" { "name" "Cyberpunk" "type" "Game" } "depots" {
            "101" { "manifests" { "public" { "gid" "11" "size" "9" "download" "3" } } }
        } }"#,
    );
    let dlc = info(
        900,
        r#""appinfo" { "common" { "name" "Phantom Liberty" "type" "DLC" "parent" "100" } "depots" {
            "900" { "manifests" { "public" { "gid" "90" "size" "5" "download" "2" } } }
            "101" { "manifests" { "public" { "gid" "11" "size" "9" "download" "3" } } }
        } }"#,
    );
    let music = info(
        950,
        r#""appinfo" { "common" { "type" "Music" "parent" "100" } "depots" {
            "950" { "manifests" { "public" { "gid" "95" "size" "1" "download" "1" } } }
        } }"#,
    );
    let lib = Library::new(vec![music, dlc, game.clone()], BTreeSet::from([101, 900, 950]));
    assert_eq!(lib.apps.iter().map(|a| a.id).collect::<Vec<_>>(), [100, 900, 950], "sorted");
    let got: Vec<(u32, u32)> = lib.install(&game, &Target::default()).iter().map(|(o, d)| (*o, d.id)).collect();
    assert_eq!(got, [(100, 101), (900, 900)], "the DLC's depot, fetched as the DLC's; each depot once; no soundtrack");
    assert_eq!(lib.depot_app.get(&900), Some(&100), "filed under its game");
    assert_eq!(lib.depot_app.get(&950), Some(&950), "a soundtrack stays its own");
    let without = Library::new(vec![lib.app(100).unwrap().clone()], BTreeSet::from([101]));
    assert_eq!(without.install(&game, &Target::default()).len(), 1, "a DLC not owned is not installed");
}

#[test]
fn an_install_takes_only_the_depots_the_licenses_grant() {
    use std::collections::BTreeSet;

    use lanstash_steam_app::library::Library;
    use lanstash_steam_domain::apps::{Target, app};
    use lanstash_steam_domain::kv::parse_text;
    let info = |id: u32, text: &str| app(id, &parse_text(text.as_bytes()).unwrap());
    // Game 700: its content (701), its own id as a depot (700, granted as an app), and 801,
    // which app 800 provides and the game lists with a manifest of its own; Steam refuses
    // such a depot ("access denied") to an account no license of which grants it.
    let game = info(
        700,
        r#""appinfo" { "common" { "type" "Game" } "depots" {
            "701" { "config" { "oslist" "windows" } "manifests" { "public" { "gid" "1" "size" "9" "download" "3" } } }
            "801" { "depotfromapp" "800" "manifests" { "public" { "gid" "2" "size" "6" "download" "6" } } }
            "700" { "manifests" { "public" { "gid" "3" "size" "1" "download" "1" } } }
        } }"#,
    );
    let ids = |lib: &Library| -> Vec<(u32, u32)> {
        lib.install(lib.app(700).unwrap(), &Target::default()).iter().map(|(o, d)| (*o, d.id)).collect()
    };
    let lib = Library::new(vec![game.clone()], BTreeSet::from([701]));
    assert_eq!(ids(&lib), [(700, 701), (700, 700)], "801 is granted by no license");
    assert!(lib.grants(700) && !lib.grants(801), "a depot granted as an app counts");
    let granted = Library::new(vec![game], BTreeSet::from([701, 801]));
    assert_eq!(
        ids(&granted),
        [(700, 701), (700, 700), (800, 801)],
        "a granted shared depot is fetched as its provider, with the manifest the game lists"
    );
    let nothing = Library::new(Vec::new(), BTreeSet::new());
    assert!(!nothing.grants(701));
}

/// Two accounts' answers: one has the game and its DLC's depot, the other the game alone.
fn two_accounts() -> Vec<(String, lanstash_steam_app::raw::Raw)> {
    use lanstash_steam_app::raw::Raw;
    let game = br#""appinfo" { "common" { "name" "Game" "type" "Game" } "depots" {
        "101" { "manifests" { "public" { "gid" "11" "size" "9" "download" "3" } } }
        "102" { "manifests" { "public" { "gid" "12" "size" "1" "download" "1" } } }
    } }"#
        .to_vec();
    let other = br#""appinfo" { "common" { "name" "Other" "type" "Game" } }"#.to_vec();
    vec![
        ("player_one".into(), Raw { owned: vec![101], apps: vec![(100, game.clone())] }),
        ("player_two".into(), Raw { owned: vec![101, 102], apps: vec![(100, game), (200, other)] }),
    ]
}

#[test]
fn several_accounts_libraries_merge_with_their_owners() {
    use lanstash_steam_app::library::from_raws;
    let lib = from_raws(&two_accounts());
    assert_eq!(lib.apps.iter().map(|a| a.id).collect::<Vec<_>>(), [100, 200], "each app once");
    assert_eq!(lib.app_owners[&100], ["player_one", "player_two"]);
    assert_eq!(lib.app_owners[&200], ["player_two"]);
    assert_eq!(lib.depot_owners[&102], ["player_two"]);
    assert_eq!(lib.owned_depots.iter().copied().collect::<Vec<_>>(), [101, 102]);
    let signed_in = ["player_one".to_string(), "player_two".to_string()];
    assert_eq!(lib.account_for(100, 101, &signed_in).as_deref(), Some("player_one"), "the first owner");
    assert_eq!(lib.account_for(100, 102, &signed_in).as_deref(), Some("player_two"), "the depot's owner");
    assert_eq!(lib.account_for(200, 999, &signed_in).as_deref(), Some("player_two"), "else the app's");
    assert_eq!(lib.account_for(300, 999, &signed_in).as_deref(), Some("player_one"), "else the first");
    assert_eq!(lib.account_for(100, 101, &[]), None);
    let game = lib.app(100).unwrap();
    let install = lib.install(game, &lanstash_steam_domain::apps::Target::default());
    let groups: Vec<(String, Vec<u32>)> = lib
        .by_account(&install, &signed_in)
        .into_iter()
        .map(|(a, ds)| (a, ds.iter().map(|(_, d)| d.id).collect()))
        .collect();
    assert_eq!(groups, [("player_one".to_string(), vec![101]), ("player_two".to_string(), vec![102])]);
}
