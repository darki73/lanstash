use lanstash_ports::platform::{BuildStatus, ClientOs, Platform, ViewError};

#[derive(Debug)]
struct Bare;

impl Platform for Bare {
    fn id(&self) -> &str {
        "bare"
    }
}

#[derive(Debug)]
struct Views;

impl Platform for Views {
    fn id(&self) -> &str {
        "views"
    }
    fn game(&self, game: &str) -> Option<Result<String, ViewError>> {
        Some(if game == "x" { Ok("{}".into()) } else { Err(ViewError::NotFound(game.into())) })
    }
}

#[test]
fn every_capability_is_optional() {
    let p: &dyn Platform = &Bare;
    assert_eq!(p.id(), "bare");
    assert!(p.names().is_none() && p.art().is_none() && p.prefill().is_none());
    assert!(p.builds().is_none() && p.catalog().is_none() && p.game("x").is_none() && p.nginx("x").is_none());
    assert!(p.setup(ClientOs::Windows).is_none());
}

#[derive(Debug)]
struct Built;

impl Platform for Built {
    fn id(&self) -> &str {
        "built"
    }
    fn builds(&self) -> Option<Result<String, ViewError>> {
        Some(Ok("{\"games\":[]}".into()))
    }
    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        Some(vec![BuildStatus { game: "g".into(), version: "1".into(), known: true, current: false }])
    }
}

#[test]
fn the_builds_and_their_status_together_are_each_as_asked_alone() {
    assert_eq!(Built.builds_and_status(), (Built.builds(), Built.build_status()));
    assert_eq!(Bare.builds_and_status(), (None, None));
}

#[test]
fn views_are_documents_or_reasons() {
    assert_eq!(Views.game("x"), Some(Ok("{}".into())));
    assert_eq!(Views.game("y"), Some(Err(ViewError::NotFound("y".into()))));
}

#[test]
fn client_systems_go_by_their_path_names() {
    for os in [ClientOs::Windows, ClientOs::MacOs, ClientOs::Linux] {
        assert_eq!(ClientOs::parse(os.name()), Some(os));
    }
    assert_eq!(ClientOs::MacOs.name(), "macos");
    assert_eq!(ClientOs::parse("Windows"), None, "names are lower case");
    assert_eq!(ClientOs::parse(""), None);
}
