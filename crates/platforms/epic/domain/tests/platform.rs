//! Tests for `lanstash_epic::platform`.

use std::sync::Arc;

use lanstash_epic::platform::EpicPlatform;
use lanstash_epic::setup::{FILE_NAME, windows_cmd};
use lanstash_ports::account::Account;
use lanstash_ports::platform::{ClientOs, Platform};

#[test]
fn epic_routes_and_sets_up_windows_pcs() {
    let p = EpicPlatform::default();
    assert_eq!(p.id(), "epicgames");
    assert!(p.art().is_none() && p.names().is_none() && p.builds().is_none() && p.prefill().is_none());
    assert!(p.account().is_none(), "none until given");
    let file = p.setup(ClientOs::Windows).expect("a Windows file");
    assert_eq!(file.name, FILE_NAME);
    assert_eq!(file.body, windows_cmd().into_bytes());
    assert!(p.setup(ClientOs::MacOs).is_none() && p.setup(ClientOs::Linux).is_none(), "not made yet");
}

struct Someone;

impl Account for Someone {
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
fn prune_and_a_games_directories_when_given() {
    let p = EpicPlatform::default();
    assert!(p.prune("app").is_none() && p.parts("app").is_none(), "none until given");
    let p = p
        .with_prune(Arc::new(|game: &str| Ok((format!("{{\"game\":\"{game}\"}}"), Vec::new()))))
        .with_parts(Arc::new(|game: &str| (game == "app").then(|| vec!["a8e6".to_string()])));
    assert_eq!(p.prune("app").unwrap().unwrap().0, "{\"game\":\"app\"}");
    assert_eq!(p.parts("app"), Some(vec!["a8e6".to_string()]), "Epic files an app's downloads under another id");
    assert_eq!(p.parts("other"), None);
}

#[test]
fn the_builds_mark_is_the_one_given() {
    assert_eq!(EpicPlatform::default().builds_mark(), 0, "none given: never moves");
    let p = EpicPlatform::default().with_builds_mark(Arc::new(|| 7));
    assert_eq!(p.builds_mark(), 7);
}

#[test]
fn a_given_account_is_offered() {
    let p = EpicPlatform::default().with_account(Arc::new(Someone));
    assert_eq!(p.account().unwrap().view(), "{}");
    assert!(p.account().unwrap().sign_in_with_code("x").is_err(), "the port's default");
    assert!(format!("{p:?}").contains("account: true"));
}
