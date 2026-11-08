//! Tests for `lanstash_epic_app::library`, against a scripted Epic.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash_epic_app::accounts::{EpicAccounts, Wiring};
use lanstash_epic_app::library::{EpicLibrary, KEPT};
use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::clock::FixedClock;
use lanstash_ports::keep::Keep;
use lanstash_ports::names::GameNames;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};

const ONE: &str = "11111111111111111111111111111111";

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, String>>);

impl Secrets for Memory {
    fn load(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), value.into());
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

#[derive(Default)]
struct Blobs(Mutex<BTreeMap<String, Vec<u8>>>);

impl Keep for Blobs {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), data.to_vec());
        Ok(())
    }
}

/// Signs in `good1`, lists two assets, describes a game and a DLC.
#[derive(Default)]
struct Epic {
    gets: Mutex<Vec<String>>,
}

impl Web for Epic {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        let ok = |text: &str| Ok(WebResponse { status: 200, body: text.as_bytes().to_vec() });
        if req.method == "POST" {
            return ok(&format!(
                r#"{{"access_token":"a1","expires_in":28800,"refresh_token":"r1","refresh_expires":1987200,"account_id":"{ONE}","displayName":"Player One"}}"#
            ));
        }
        if req.host == "cdn1.epicgames.com" {
            assert!(req.tls.is_some() && req.port == 443, "over HTTPS");
            assert!(req.headers.is_empty(), "no account for a public image");
            return Ok(WebResponse { status: 200, body: b"\xFF\xD8\xFFjpeg".to_vec() });
        }
        assert!(req.headers.contains(&("Authorization", "bearer a1")), "as the account");
        self.gets.lock().unwrap().push(format!("{}{}", req.host, req.target));
        if req.target.starts_with("/launcher/api/public/assets/v2/") {
            return if req.target.contains("/app/gameapp/") {
                ok(
                    r#"{"elements":[{"manifests":[{"uri":"https://download.epicgames.com/Builds/Org/o-1/a8e62719f55845c0a961ce64e3e05767/default/X.manifest"}]}]}"#,
                )
            } else {
                ok(r#"{"elements":[]}"#)
            };
        }
        if req.target.starts_with("/launcher/api/public/assets/Windows") {
            return ok(
                r#"[{"appName":"gameapp","buildVersion":"7","catalogItemId":"c1","namespace":"ns"},{"appName":"dlcapp","buildVersion":"1","catalogItemId":"c2","namespace":"ns"}]"#,
            );
        }
        if req.target.starts_with("/library/api/public/items") {
            return if req.target.contains("cursor=next") {
                ok(r#"{"responseMetadata":{},"records":[]}"#)
            } else {
                ok(
                    r#"{"responseMetadata":{"nextCursor":"next"},"records":[{"appName":"gameapp","namespace":"ns","catalogItemId":"c1"},{"appName":"freeapp","namespace":"ns","catalogItemId":"c3"}]}"#,
                )
            };
        }
        if req.target.contains("id=c3") {
            return ok(r#"{"c3":{"title":"A Free Game","categories":[{"path":"games"}],"keyImages":[]}}"#);
        }
        if req.target.contains("id=c1") {
            return ok(
                r#"{"c1":{"title":"Some Game","categories":[{"path":"games"}],"keyImages":[{"type":"DieselGameBox","url":"https://cdn1.epicgames.com/wide.jpg"}]}}"#,
            );
        }
        ok(r#"{"c2":{"title":"Some DLC","categories":[{"path":"addons"}],"mainGameItem":{"id":"c1"}}}"#)
    }
}

fn library(keep: Arc<Blobs>) -> (Arc<EpicLibrary>, Arc<Epic>) {
    let web = Arc::new(Epic::default());
    let wiring = Wiring {
        web: web.clone(),
        secrets: Arc::new(Memory::default()),
        clock: Arc::new(FixedClock(1_000)),
        client_id: "id".into(),
        client_secret: "secret".into(),
    };
    let accounts = Arc::new(EpicAccounts::new(wiring));
    accounts.add("good1").unwrap();
    let lib = EpicLibrary::new(accounts, web.clone(), Arc::new(FixedClock(2_000))).with_keep(keep);
    (Arc::new(lib), web)
}

#[test]
fn the_library_names_the_games_finds_their_art_and_lists_them_with_their_owners() {
    let (lib, web) = library(Arc::new(Blobs::default()));
    assert!(lib.view(&|_| None).contains("\"error\":\"reading the Epic library\""), "nothing read yet");
    lib.refresh().unwrap();
    assert_eq!(
        web.gets.lock().unwrap().len(),
        8,
        "the assets, two pages of the library, each item once, each built app's manifests once"
    );
    let dir = "a8e62719f55845c0a961ce64e3e05767";
    assert_eq!(lib.name("epicgames", dir).as_deref(), Some("Some Game"), "a download directory names its app");
    assert!(lib.locate(dir, ArtKind::Header).is_ok(), "and finds its art");
    assert_eq!(lib.name("epicgames", "GAMEAPP").as_deref(), Some("Some Game"), "any case");
    assert_eq!(lib.name("epicgames", "freeapp").as_deref(), Some("A Free Game"), "from the library service");
    assert_eq!(lib.fetch("gameapp", ArtKind::Header), Some(Ok(b"\xFF\xD8\xFFjpeg".to_vec())), "over HTTPS itself");
    assert_eq!(lib.fetch("freeapp", ArtKind::Header), Some(Err(ArtError::Missing)), "no image");
    assert_eq!(lib.name("steam", "gameapp"), None);
    let art = lib.locate("gameapp", ArtKind::Header).unwrap();
    assert_eq!((art.host.as_str(), art.target.as_str()), ("cdn1.epicgames.com", "/wide.jpg"));
    assert_eq!(lib.locate("gameapp", ArtKind::Logo), Err(ArtError::Missing));
    assert_eq!(lib.locate("nothing", ArtKind::Header), Err(ArtError::Unknown));
    let v = lib.view(&|_| None);
    assert!(
        v.contains(r#"{"code":"gameapp","name":"Some Game","game":"gameapp","version":"7","known":true,"owners":["Player One"]}"#),
        "{v}"
    );
    assert!(!v.contains("Some DLC"), "a DLC is not a game of its own");
    let sized = lib.view(&|a| (a.app_name == "gameapp").then_some(9_000));
    assert!(sized.contains(r#""version":"7","known":true,"download":9000,"owners""#), "{sized}");
    let games: Vec<String> = lib.games().into_iter().map(|a| a.app_name).collect();
    assert!(games.contains(&"gameapp".to_string()) && games.contains(&"freeapp".to_string()), "{games:?}");
    assert!(!games.iter().any(|g| g.contains("dlc")), "games only: {games:?}");
    let before = web.gets.lock().unwrap().len();
    lib.refresh().unwrap();
    assert_eq!(web.gets.lock().unwrap().len(), before + 3, "items are asked for once: the assets and the pages only");
}

#[test]
fn the_library_is_there_at_once_after_a_start() {
    let keep = Arc::new(Blobs::default());
    let (lib, _) = library(keep.clone());
    lib.refresh().unwrap();
    assert!(keep.0.lock().unwrap().contains_key(KEPT));
    let (again, web) = library(keep);
    assert_eq!(again.name("epicgames", "gameapp").as_deref(), Some("Some Game"), "from the kept read");
    assert_eq!(
        again.name("epicgames", "a8e62719f55845c0a961ce64e3e05767").as_deref(),
        Some("Some Game"),
        "the directories kept too"
    );
    assert!(again.view(&|_| None).contains("\"Some Game\""));
    assert!(web.gets.lock().unwrap().is_empty(), "nothing asked of Epic");
    again.find_dirs();
    again.find_dirs();
    let gets = web.gets.lock().unwrap();
    assert_eq!(gets.len(), 1, "only the app whose directory was not found, once a run: {gets:?}");
    assert!(gets[0].contains("/app/dlcapp/"));
}
