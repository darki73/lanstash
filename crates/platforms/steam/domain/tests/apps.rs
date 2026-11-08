//! Tests for `lanstash_steam_domain::apps`, on app info shaped like
//! Deadlock's (values made up here).

use lanstash_steam_domain::apps::{Target, app};
use lanstash_steam_domain::kv::parse_text;

const INFO: &str = r#"
"appinfo"
{
    "appid" "100"
    "common" { "name" "Some Game" "type" "Game" }
    "depots"
    {
        "228989" { "config" { "oslist" "windows" } "depotfromapp" "228980" "sharedinstall" "1" }
        "101" { "manifests" { "public" { "gid" "5350434913634208315" "size" "468926722" "download" "159418544" } } }
        "102" { "config" { "oslist" "windows" } "manifests" { "public" { "gid" "7" "size" "700" "download" "250" } } }
        "103" { "config" { "oslist" "linux" } "manifests" { "public" { "gid" "8" "size" "1" "download" "1" } } }
        "104" { "config" { "oslist" "windows,macos" "language" "german" } "manifests" { "public" { "gid" "9" "size" "5" "download" "5" } } }
        "105" { "config" { "osarch" "32" } "manifests" { "public" "10" } }
        "106" { "dlcappid" "200" "manifests" { "public" { "gid" "11" "size" "5" "download" "5" } } }
        "branches" { "public" { "buildid" "26467182" } }
        "baselanguages" "english"
    }
}
"#;

#[test]
fn reads_the_name_build_and_depots() {
    let a = app(100, &parse_text(INFO.as_bytes()).unwrap());
    assert_eq!((a.title().as_str(), a.kind.as_deref(), a.build), ("Some Game", Some("Game"), Some(26_467_182)));
    assert_eq!(a.depots.len(), 7, "branches and baselanguages are not depots");
    let shared = &a.depots[0];
    assert_eq!((shared.id, shared.manifest, shared.from_app), (228_989, None, Some(228_980)));
    let main = &a.depots[1];
    assert_eq!((main.manifest, main.size, main.download), (Some(5_350_434_913_634_208_315), 468_926_722, 159_418_544));
    assert_eq!(a.depots[4].oslist, ["windows", "macos"]);
    assert_eq!(a.depots[5].manifest, Some(10), "the older form: the gid right under public");
}

#[test]
fn an_empty_placeholder_depot_is_not_installed_but_one_without_sizes_is() {
    // 902 has a manifest of nothing, and asking for it is refused ("access denied").
    // An older answer gives no sizes at all.
    let info = r#"
"appinfo"
{
    "depots"
    {
        "901" { "config" { "oslist" "windows" } "manifests" { "public" { "gid" "11" "size" "20010870481" "download" "19167841408" } } }
        "902" { "manifests" { "public" { "gid" "12" "size" "0" "download" "0" } } }
        "903" { "manifests" { "public" "777" } }
    }
}
"#;
    let a = app(900, &parse_text(info.as_bytes()).unwrap());
    let ids: Vec<u32> = a.install(&Target::default()).iter().map(|d| d.id).collect();
    assert_eq!(ids, [901, 903]);
    let empty = a.depots.iter().find(|d| d.id == 902).unwrap();
    assert!(empty.empty && !empty.has_content());
    assert!(!a.depots.iter().find(|d| d.id == 903).unwrap().empty, "no sizes given: not known to be empty");
}

#[test]
fn a_depots_download_is_estimated_from_steams_figures_where_they_are_off() {
    // As older apps' info has them: a real size beside a `download` of 0, or a download far past it.
    let info = r#"
"appinfo"
{
    "depots"
    {
        "301" { "manifests" { "public" { "gid" "1" "size" "5042891914" "download" "0" } } }
        "302" { "manifests" { "public" { "gid" "2" "size" "900" "download" "400" } } }
        "303" { "manifests" { "public" { "gid" "3" "size" "70" } } }
        "304" { "manifests" { "public" { "gid" "4" "size" "3310688106" "download" "189836539520" } } }
        "305" { "manifests" { "public" { "gid" "5" "download" "60" } } }
    }
}
"#;
    let a = app(300, &parse_text(info.as_bytes()).unwrap());
    let download = |id: u32| a.depots.iter().find(|d| d.id == id).unwrap().download;
    assert_eq!(download(301), 5_042_891_914, "0 given: its size");
    assert_eq!((download(302), download(303), download(305)), (400, 70, 60), "as given, or the one figure there is");
    assert_eq!(download(304), 3_310_688_106, "never more than it installs");
    assert!(!a.depots[0].empty, "a size: not a placeholder");
    assert_eq!(a.download(&Target::default()), 5_042_891_914 + 400 + 70 + 3_310_688_106 + 60);
}

#[test]
fn an_install_takes_the_depots_for_its_target() {
    let a = app(100, &parse_text(INFO.as_bytes()).unwrap());
    let ids = |t: &Target| a.install(t).iter().map(|d| d.id).collect::<Vec<_>>();
    assert_eq!(ids(&Target::default()), [101, 102]);
    assert_eq!(a.download(&Target::default()), 159_418_544 + 250);
    assert_eq!(a.shared(&Target::default()), [(228_980, 228_989)], "the redistributable another app provides");
    let dlc: Vec<(u32, u32)> = a.dlc_depots(&Target::default()).iter().map(|(x, d)| (*x, d.id)).collect();
    assert_eq!(dlc, [(200, 106)], "the DLC's depot, for whoever owns DLC 200");
    let linux = Target { os: "linux".into(), ..Target::default() };
    assert!(a.shared(&linux).is_empty(), "it is for Windows");
    let german = Target { language: "german".into(), ..Target::default() };
    assert_eq!(ids(&german), [101, 102, 104]);
    let both = Target { language: "english, German".into(), ..Target::default() };
    assert_eq!(ids(&both), [101, 102, 104], "each language's depots");
    assert!(both.speaks("german") && both.speaks("ENGLISH") && !both.speaks("french"));
    let linux32 = Target { os: "linux".into(), language: "english".into(), arch: "32".into() };
    assert_eq!(ids(&linux32), [101, 103, 105]);
}

#[test]
fn a_dlc_with_depots_of_its_own_belongs_to_its_game() {
    let info = |id: u32, text: &str| app(id, &parse_text(text.as_bytes()).unwrap());
    let game = info(100, r#""appinfo" { "common" { "type" "Game" } "extended" { "listofdlc" "300, 400" } }"#);
    assert_eq!(game.dlcs, [300, 400]);
    // Shaped like Cyberpunk 2077: Phantom Liberty: its content is a depot of its own.
    let dlc = info(
        200,
        r#""appinfo" { "common" { "type" "DLC" "parent" "100" } "depots" {
            "200" { "manifests" { "public" { "gid" "21" "size" "9" "download" "4" } } }
            "201" { "config" { "language" "german" } "manifests" { "public" { "gid" "22" "size" "1" "download" "1" } } }
            "228989" { "depotfromapp" "228980" }
            "202" { "dlcappid" "999" "manifests" { "public" { "gid" "23" "size" "1" "download" "1" } } }
        } }"#,
    );
    assert_eq!(dlc.parent, Some(100));
    assert!(dlc.is_dlc_of(&game), "by its parent");
    assert_eq!(dlc.dlc_content(&Target::default()).iter().map(|d| d.id).collect::<Vec<_>>(), [200]);
    let listed = info(300, r#""appinfo" { "common" { "type" "DLC" } }"#);
    assert!(listed.is_dlc_of(&game), "listed by the game");
    let soundtrack = info(400, r#""appinfo" { "common" { "type" "Music" "parent" "100" } }"#);
    assert!(!soundtrack.is_dlc_of(&game), "a soundtrack is not installed with the game");
    assert!(!game.is_dlc_of(&game));
}

#[test]
fn an_app_without_info_is_named_by_its_id() {
    let a = app(7, &parse_text(b"\"appinfo\" { }").unwrap());
    assert_eq!((a.title().as_str(), a.build, a.depots.len()), ("7", None, 0));
}

#[test]
fn reads_the_store_arts_file_names() {
    let info = r#"
"appinfo"
{
    "common"
    {
        "name" "New Game"
        "icon" "6C1D20C62C4613263548323052C62CECE488876B"
        "header_image" { "english" "80132dfeee2f6463f4c71821edf426af6e8fed97/header.jpg" "german" "x/header_german.jpg" }
        "library_assets_full"
        {
            "library_hero" { "image" { "english" "aa11/library_hero.jpg" } }
            "library_logo" { "image" { "german" "bb22/logo_german.png" } "logo_position" { "pinned_position" "BottomLeft" } }
        }
    }
}
"#;
    let a = app(3_215_050, &parse_text(info.as_bytes()).unwrap());
    assert_eq!(a.art.header.as_deref(), Some("80132dfeee2f6463f4c71821edf426af6e8fed97/header.jpg"));
    assert_eq!(a.art.hero.as_deref(), Some("aa11/library_hero.jpg"));
    assert_eq!(a.art.logo.as_deref(), Some("bb22/logo_german.png"), "no English one: the first there is");
    assert_eq!(a.art.icon.as_deref(), Some("6c1d20c62c4613263548323052c62cece488876b"), "lower-cased");
    let odd = r#""appinfo" { "common" { "header_image" { "english" "../../etc/passwd" } "icon" "../x" } }"#;
    let odd = app(1, &parse_text(odd.as_bytes()).unwrap());
    assert_eq!((odd.art.header, odd.art.icon), (None, None), "not a safe path, not a SHA-1");
    assert_eq!(app(1, &parse_text(INFO.as_bytes()).unwrap()).art, Default::default(), "none named");
}
