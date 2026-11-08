//! Tests for `lanstash_steam_domain::art`.

use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_steam_domain::art::{PAGE_BACKGROUND, STEAM_ART_HOST, SteamArt, is_app_id};

#[test]
fn headers_by_app_id_only() {
    let s = SteamArt::default().locate("1422450", ArtKind::Header).unwrap();
    assert_eq!((s.host.as_str(), s.port, s.target.as_str()), (STEAM_ART_HOST, None, "/steam/apps/1422450/header.jpg"));
    assert_eq!(SteamArt::default().locate("1422450", ArtKind::Icon), Err(ArtError::Unknown), "no app info: no icon");
    let hero = SteamArt::default().locate("1422450", ArtKind::Background).unwrap();
    assert_eq!(hero.target, "/steam/apps/1422450/library_hero.jpg");
    assert_eq!(SteamArt::default().locate("1422450", ArtKind::Logo).unwrap().target, "/steam/apps/1422450/logo.png");
    assert_eq!(SteamArt::default().locate("x", ArtKind::Logo), Err(ArtError::Unknown), "not an app id");
    for bad in ["", "0", "01", "../etc", "12345678901", "12a"] {
        assert!(!is_app_id(bad), "{bad}");
        assert_eq!(SteamArt::default().locate(bad, ArtKind::Header), Err(ArtError::Unknown), "{bad}");
    }
}

#[test]
fn an_older_game_without_a_hero_falls_back_on_its_page_background() {
    let art = SteamArt::default();
    let bg = art.fallback("292410", ArtKind::Background).unwrap();
    assert_eq!(
        (bg.host.as_str(), bg.target.as_str()),
        (STEAM_ART_HOST, "/steam/apps/292410/page_bg_generated_v6b.jpg")
    );
    assert_eq!(bg.target.rsplit('/').next(), Some(PAGE_BACKGROUND));
    assert!(art.fallback("292410", ArtKind::Header).is_none(), "only the background has one");
    assert!(art.fallback("../x", ArtKind::Background).is_none());
}

#[test]
fn the_files_app_info_names_come_first() {
    use lanstash_steam_domain::apps::Assets;
    use std::sync::Arc;
    let named = Assets {
        header: Some("80132dfe/header.jpg".into()),
        hero: None,
        logo: Some("abc/logo.png".into()),
        icon: Some("6c1d20c62c4613263548323052c62cece488876b".into()),
    };
    let art = SteamArt::default().with_assets(Arc::new(move |app| (app == 3_215_050).then(|| named.clone())));
    let s = art.locate("3215050", ArtKind::Header).unwrap();
    assert_eq!(
        (s.host.as_str(), s.target.as_str()),
        (lanstash_steam_domain::art::STEAM_ASSET_HOST, "/store_item_assets/steam/apps/3215050/80132dfe/header.jpg")
    );
    assert_eq!(
        art.locate("3215050", ArtKind::Logo).unwrap().target,
        "/store_item_assets/steam/apps/3215050/abc/logo.png"
    );
    assert_eq!(
        art.locate("3215050", ArtKind::Background).unwrap().target,
        "/steam/apps/3215050/library_hero.jpg",
        "none named"
    );
    assert_eq!(art.locate("7", ArtKind::Header).unwrap().target, "/steam/apps/7/header.jpg", "unknown app");
    // The icon app info names, among the community images.
    let icon = art.locate("3215050", ArtKind::Icon).unwrap();
    assert_eq!(
        (icon.host.as_str(), icon.target.as_str()),
        (STEAM_ART_HOST, "/steamcommunity/public/images/apps/3215050/6c1d20c62c4613263548323052c62cece488876b.jpg")
    );
    assert_eq!(art.locate("7", ArtKind::Icon), Err(ArtError::Unknown), "none named: no icon");
}
