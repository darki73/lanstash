//! Tests for `lanstash_epic::library`.

use lanstash_domain::json::{Value, parse};
use lanstash_epic::library::{
    Asset, Game, Item, image, image_source, items_path, parse_assets, parse_items, render_catalog, render_none,
};
use lanstash_ports::art::ArtKind;

const ASSETS: &str = r#"[
  {"appName":"a8e62719f55845c0a961ce64e3e05767","labelName":"Live-Windows","buildVersion":"1.2.3","catalogItemId":"c1","namespace":"ns1","assetId":"a8e62719f55845c0a961ce64e3e05767"},
  {"appName":"Fortnite","buildVersion":"++Fortnite+Release","catalogItemId":"c2","namespace":"fn"},
  {"appName":"../evil","catalogItemId":"c3","namespace":"ns3"},
  {"appName":"NoCatalog","namespace":"ns4"}
]"#;

#[test]
fn an_accounts_assets_are_read_and_unsafe_ones_left_out() {
    let list = parse_assets(ASSETS).unwrap();
    assert_eq!(
        list.iter().map(|a| a.app_name.as_str()).collect::<Vec<_>>(),
        ["a8e62719f55845c0a961ce64e3e05767", "Fortnite"]
    );
    assert_eq!(
        (list[0].namespace.as_str(), list[0].catalog_item_id.as_str(), list[0].build_version.as_str()),
        ("ns1", "c1", "1.2.3")
    );
    assert_eq!(parse_assets(r#"{"errorMessage":"Sorry, no access"}"#).unwrap_err(), "Epic: Sorry, no access");
    assert!(parse_assets("<html>").is_err());
}

#[test]
fn an_item_is_asked_for_by_its_namespace_and_id() {
    assert_eq!(
        items_path("ns1", "c1").as_deref(),
        Some(
            "/catalog/api/shared/namespace/ns1/bulk/items?id=c1&includeDLCDetails=true&includeMainGameDetails=true&country=US&locale=en"
        )
    );
    assert_eq!(items_path("ns/1", "c1"), None);
    assert_eq!(items_path("ns1", "c1&x=y"), None);
}

#[test]
fn catalog_items_say_their_title_whether_they_are_games_and_their_images() {
    let body = r#"{
      "c1": {"id":"c1","title":"Some Game","categories":[{"path":"games"},{"path":"games/edition/base"}],
             "keyImages":[{"type":"DieselGameBoxTall","url":"https://cdn1.epicgames.com/tall.jpg"},{"type":"DieselGameBox","url":"https://cdn1.epicgames.com/wide.jpg"}]},
      "c2": {"id":"c2","title":"Some DLC","categories":[{"path":"addons"},{"path":"games"}],"mainGameItem":{"id":"c1"}},
      "c3": {"id":"c3","title":"An engine asset","categories":[{"path":"assets"}]},
      "c4": {"id":"c4","title":"A tool","categories":[{"path":"applications"}],"mainGameItem":null}
    }"#;
    let items = parse_items(body).unwrap();
    let game = |id: &str| items.iter().find(|(i, _)| i == id).map(|(_, x)| x.game);
    assert_eq!((game("c1"), game("c2"), game("c3"), game("c4")), (Some(true), Some(false), Some(false), Some(true)));
    let (_, some) = &items[0];
    assert_eq!(some.title, "Some Game");
    assert_eq!(image(some, ArtKind::Header), Some("https://cdn1.epicgames.com/wide.jpg"));
    assert_eq!(
        image(some, ArtKind::Icon),
        Some("https://cdn1.epicgames.com/tall.jpg"),
        "the tall one, without a thumbnail"
    );
    assert_eq!(image(some, ArtKind::Logo), None);
    assert!(parse_items(r#"{"errorMessage":"nope"}"#).is_err());
}

#[test]
fn an_image_comes_from_epics_cdn_only() {
    let s = image_source("https://cdn1.epicgames.com/offer/x/wide.jpg?h=1").unwrap();
    assert_eq!((s.host.as_str(), s.port, s.target.as_str()), ("cdn1.epicgames.com", None, "/offer/x/wide.jpg?h=1"));
    assert!(image_source("https://example.com/x.jpg").is_none(), "not Epic's");
    assert!(image_source("ftp://cdn1.epicgames.com/x").is_none());
}

#[test]
fn the_catalog_view_lists_each_game_by_title_with_its_owners() {
    let a = Asset {
        app_name: "zeta".into(),
        namespace: "n".into(),
        catalog_item_id: "c".into(),
        build_version: "2".into(),
    };
    let b = Asset {
        app_name: "alpha".into(),
        namespace: "n".into(),
        catalog_item_id: "d".into(),
        build_version: "1".into(),
    };
    let games = [
        Game { asset: &a, title: "Zeta", owners: vec!["player_one".into()], download: Some(4_000) },
        Game {
            asset: &b,
            title: "alpha \"A\"",
            owners: vec!["player_one".into(), "player_two".into()],
            download: None,
        },
    ];
    let v = parse(&render_catalog(&games)).unwrap();
    let products = v.get("products").unwrap().as_array();
    let names: Vec<&str> = products.iter().filter_map(|p| p.get("name").and_then(Value::as_str)).collect();
    assert_eq!(names, ["alpha \"A\"", "Zeta"]);
    assert_eq!(products[0].get("game").and_then(Value::as_str), Some("alpha"));
    assert_eq!(products[0].get("owners").unwrap().as_array().len(), 2);
    assert!(products[0].get("download").is_none(), "not worked out yet: left out");
    assert_eq!(products[1].get("download"), Some(&Value::Number(4_000.0)));
    assert_eq!(render_none("reading"), "{\"products\":[],\"error\":\"reading\"}\n");
    let _ = Item { title: String::new(), game: false, images: vec![] };
}

#[test]
fn an_apps_download_directory_comes_from_its_manifests() {
    use lanstash_epic::library::{manifest_path, parse_manifest_dirs};
    let a = Asset {
        app_name: "Dodo".into(),
        namespace: "ns".into(),
        catalog_item_id: "c1".into(),
        build_version: "2".into(),
    };
    assert_eq!(
        manifest_path(&a).as_deref(),
        Some("/launcher/api/public/assets/v2/platform/Windows/namespace/ns/catalogItem/c1/app/Dodo/label/Live")
    );
    let body = r#"{"elements":[{"appName":"Dodo","manifests":[
        {"uri":"https://epicgames-download1.akamaized.net/Builds/Org/o-abc/a8e62719f55845c0a961ce64e3e05767/default/X.manifest","queryParams":[]},
        {"uri":"https://fastly-download.epicgames.com/Builds/Org/o-abc/a8e62719f55845c0a961ce64e3e05767/default/X.manifest"},
        {"uri":"https://download.epicgames.com/Builds/Dodo/CloudDir/Y.manifest"}]}]}"#;
    assert_eq!(parse_manifest_dirs(body), ["a8e62719f55845c0a961ce64e3e05767", "Dodo"], "each once");
    assert!(parse_manifest_dirs("{}").is_empty());
}

#[test]
fn a_builds_manifest_urls_carry_their_signature_and_directory() {
    use lanstash_epic::library::parse_manifest_urls;
    let body = r#"{"elements":[{"manifests":[
        {"uri":"https://fastly-download.epicgames.com/Builds/Org/o-1/dir1/default/M.manifest","queryParams":[{"name":"f_token","value":"a b"}]},
        {"uri":"https://download.epicgames.com/Builds/Org/o-1/dir1/default/M.manifest","queryParams":[]}]}]}"#;
    let urls = parse_manifest_urls(body);
    assert_eq!(urls.len(), 2);
    assert_eq!(
        (urls[0].host.as_str(), urls[0].target.as_str(), urls[0].dir.as_str()),
        (
            "fastly-download.epicgames.com",
            "/Builds/Org/o-1/dir1/default/M.manifest?f_token=a%20b",
            "/Builds/Org/o-1/dir1/default/"
        )
    );
    assert_eq!(urls[1].target, "/Builds/Org/o-1/dir1/default/M.manifest");
}
