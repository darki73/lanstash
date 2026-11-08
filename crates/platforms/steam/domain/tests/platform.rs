//! Tests for `lanstash_steam_domain::platform`.

use lanstash_ports::art::ArtKind;
use lanstash_ports::platform::Platform;
use lanstash_steam_domain::platform::SteamPlatform;

#[test]
fn steam_has_art_and_no_account_unless_given() {
    let p = SteamPlatform::default();
    assert_eq!(p.id(), "steam");
    assert!(p.art().unwrap().locate("7", ArtKind::Header).is_ok());
    assert!(p.names().is_none() && p.builds().is_none() && p.catalog().is_none() && p.prefill().is_none());
    assert!(p.game("7").is_none() && p.nginx("7").is_none() && p.account().is_none());
}

struct NoOne;

impl lanstash_ports::account::Account for NoOne {
    fn view(&self) -> String {
        "{}".into()
    }
    fn start_sign_in(&self) -> Result<(), String> {
        Ok(())
    }
    fn cancel_sign_in(&self) {}
    fn sign_out(&self, _account: &str) -> Result<(), String> {
        Ok(())
    }
    fn qr_svg(&self) -> Option<String> {
        None
    }
}

#[test]
fn a_given_account_is_offered() {
    let p = SteamPlatform::default().with_account(std::sync::Arc::new(NoOne));
    assert_eq!(p.account().unwrap().view(), "{}");
}

#[test]
fn a_game_names_its_parts_when_told_how() {
    assert!(SteamPlatform::default().parts("7").is_none());
    let p = SteamPlatform::default()
        .with_parts(std::sync::Arc::new(|g: &str| (g == "7").then(|| vec!["7".to_string(), "8".to_string()])));
    assert_eq!(p.parts("7"), Some(vec!["7".to_string(), "8".to_string()]));
    assert!(p.parts("9").is_none());
}
