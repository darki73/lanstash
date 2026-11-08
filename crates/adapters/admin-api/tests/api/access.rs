//! Tests for `api::access`: who may do what once lanstash has accounts.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use lanstash_adapter_admin_api::api::access::{Caller, SESSION_COOKIE, needs, session_token};
use lanstash_adapter_admin_api::route::Route;
use lanstash_app::accounts::Accounts;
use lanstash_app::hidden::HiddenGames;
use lanstash_app::invites::Invites;
use lanstash_app::sessions::Sessions;
use lanstash_domain::accounts::{Permission, Role};
use lanstash_ports::digest::Digest;
use lanstash_ports::keep::Keep;
use lanstash_ports::passwords::Passwords;
use lanstash_ports::random::Random;
use lanstash_ports::secrets::Secrets;

use super::*;

#[derive(Default)]
pub struct Memory(Mutex<BTreeMap<String, String>>);

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

impl Keep for Memory {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().get(name).map(|s| s.clone().into_bytes()))
    }
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), String::from_utf8_lossy(data).into_owned());
        Ok(())
    }
}

pub struct Plain;
impl Passwords for Plain {
    fn hash(&self, p: &str) -> Result<String, String> {
        Ok(format!("plain${}", p.replace(' ', "_")))
    }
    fn verify(&self, p: &str, kept: &str) -> bool {
        kept == format!("plain${}", p.replace(' ', "_"))
    }
}

/// Each draw a new byte.
#[derive(Default)]
pub struct Counting(Mutex<u8>);
impl Random for Counting {
    fn fill(&self, buf: &mut [u8]) -> Result<(), String> {
        let mut n = self.0.lock().unwrap();
        *n += 1;
        buf.fill(*n);
        Ok(())
    }
}

pub struct Same;
impl Digest for Same {
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (o, d) in out.iter_mut().zip(data) {
            *o = *d;
        }
        out
    }
}

/// An API with accounts: `boss` (admin) and `player.one` (viewer), passwords "long enough pw".
pub fn with_accounts() -> AdminApi {
    accounts_on(api())
}

/// `api` with the accounts of [`with_accounts`].
pub fn accounts_on(api: AdminApi) -> AdminApi {
    let secrets = Arc::new(Memory::default());
    let accounts = Arc::new(Accounts::new(secrets.clone(), Arc::new(Plain)).unwrap());
    accounts.add("boss", Role::Admin, "long enough pw").unwrap();
    accounts.add("player.one", Role::Viewer, "long enough pw").unwrap();
    let invites = Invites::new(
        accounts.clone(),
        secrets.clone(),
        Arc::new(Counting::default()),
        Arc::new(Same),
        Arc::new(FixedClock(5)),
    );
    let sessions = Sessions::new(
        accounts,
        secrets.clone(),
        Arc::new(Counting::default()),
        Arc::new(Same),
        Arc::new(FixedClock(5)),
    )
    .unwrap();
    api.with_sessions(Arc::new(sessions))
        .with_invites(Arc::new(invites))
        .with_hidden(Arc::new(HiddenGames::new(secrets).unwrap()))
}

/// One request with `headers`, from this machine or from another.
pub fn send(api: &AdminApi, method: &str, target: &str, headers: &[(&str, &str)], local: bool) -> Answer {
    let req = RequestHead {
        method: method.into(),
        target: target.into(),
        minor_version: 1,
        headers: headers.iter().map(|(n, v)| ((*n).into(), (*v).into())).collect(),
    };
    let mut c = Capture::default();
    let mut conn = ConnState::new(local).with_client(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7)));
    api.handle(&req, &mut conn, &mut c).unwrap();
    Answer { status: c.status, headers: c.headers, body: String::from_utf8_lossy(&c.body).into_owned() }
}

/// Signs `name` in from another machine: its cookie header.
pub fn cookie_of(api: &AdminApi, name: &str) -> String {
    let a = send(
        api,
        "POST",
        "/api/v1/session",
        &[("x-lanstash-user", name), ("x-lanstash-password", "long+enough+pw")],
        false,
    );
    assert_eq!(a.status, 200, "{}", a.body);
    let set = a.headers.iter().find(|(n, _)| n == "Set-Cookie").unwrap().1.clone();
    set.split(';').next().unwrap().to_string()
}

#[test]
fn a_guest_and_a_viewer_read_but_only_an_admin_changes() {
    let a = with_accounts();
    let viewer = cookie_of(&a, "player.one");
    let admin = cookie_of(&a, "boss");
    assert_eq!(send(&a, "GET", "/api/v1/status", &[], false).status, 200, "reading is open");
    assert_eq!(send(&a, "PUT", "/api/v1/hidden/steam/1", &[], false).status, 401, "a guest signs in first");
    assert_eq!(send(&a, "PUT", "/api/v1/hidden/steam/1", &[("cookie", &viewer)], false).status, 403);
    assert_eq!(send(&a, "GET", "/api/v1/hidden", &[("cookie", &viewer)], false).status, 403, "the list names games");
    assert_eq!(send(&a, "PUT", "/api/v1/hidden/steam/1", &[("cookie", &admin)], false).status, 200);
    assert_eq!(send(&a, "PUT", "/api/v1/hidden/steam/2", &[], true).status, 200, "this machine is an admin");
}

#[test]
fn hidden_games_are_masked_for_all_but_an_admin() {
    let a = with_accounts().with_platforms(vec![Arc::new(Echo { id: "steam", views: true, jobs: None })]);
    assert_eq!(send(&a, "PUT", "/api/v1/hidden/steam/10", &[], true).status, 200);
    let viewer = cookie_of(&a, "player.one");
    for who in [vec![], vec![("cookie", viewer.as_str())]] {
        assert_eq!(send(&a, "GET", "/api/v1/games/steam/10", &who, false).status, 404, "its page is not found");
        assert_eq!(send(&a, "GET", "/api/v1/art/steam/10/icon", &who, false).status, 404, "nor its art");
        assert_eq!(send(&a, "GET", "/api/v1/games/steam/20", &who, false).body, "{\"game\":\"20\"}\n");
    }
    let admin = cookie_of(&a, "boss");
    assert_eq!(send(&a, "GET", "/api/v1/games/steam/10", &[("cookie", &admin)], false).body, "{\"game\":\"10\"}\n");
    assert_eq!(send(&a, "GET", "/api/v1/games/steam/10", &[], true).body, "{\"game\":\"10\"}\n", "this machine too");
}

#[test]
fn the_dns_servers_state_needs_dns_and_who_the_clients_are_needs_a_sign_in() {
    let a = with_accounts();
    let viewer = cookie_of(&a, "player.one");
    let admin = cookie_of(&a, "boss");
    let status = |path: &str, c: Option<&str>| {
        let headers: Vec<(&str, &str)> = c.map(|c| vec![("cookie", c)]).unwrap_or_default();
        send(&a, "GET", path, &headers, false).status
    };
    assert_eq!(status("/api/v1/dns", None), 401);
    assert_eq!(status("/api/v1/dns", Some(&viewer)), 403);
    assert_eq!(status("/api/v1/dns", Some(&admin)), 404, "allowed: this lanstash has no DNS link");
    assert_eq!(status("/api/v1/clients?from=0&to=1", None), 401);
    assert_eq!(status("/api/v1/clients?from=0&to=1", Some(&viewer)), 404, "allowed: no clients kept here");
    assert_eq!(status("/api/v1/stats/top?prefix=client%2F&from=0&to=1", None), 401);
    assert_eq!(status("/api/v1/stats?names=client/192.0.2.7/steam/1/hit&from=0&to=1", None), 401);
    assert_eq!(status("/api/v1/stats/top?prefix=served%2F&from=0&to=1", None), 404, "the games served stay open");
    assert_eq!(status("/api/v1/stats/top?prefix=CLIENT%2f&from=0&to=1", Some(&viewer)), 404);
    let guest = send(&a, "GET", "/api/v1/status", &[], false).body;
    assert!(guest.contains("\"clients\":[]"), "{guest}");
}

#[test]
fn without_accounts_everything_is_answered_as_before() {
    let open = api().with_hidden(Arc::new(HiddenGames::new(Arc::new(Memory::default())).unwrap()));
    assert_eq!(send(&open, "PUT", "/api/v1/hidden/steam/1", &[], false).status, 200);
}

#[test]
fn the_guard_still_comes_first() {
    let a = with_accounts();
    let admin = cookie_of(&a, "boss");
    let other = [("cookie", admin.as_str()), ("origin", "http://evil.example"), ("host", "10.0.0.2:8081")];
    assert_eq!(send(&a, "PUT", "/api/v1/hidden/steam/1", &other, false).status, 403);
}

#[test]
fn the_session_cookie_is_found_among_others() {
    let req = |c: &str| RequestHead {
        method: "GET".into(),
        target: "/".into(),
        minor_version: 1,
        headers: vec![("cookie".into(), c.into())],
    };
    assert_eq!(session_token(&req(&format!("a=1; {SESSION_COOKIE}=abc; b=2"))), Some("abc"));
    assert_eq!(session_token(&req(&format!("{SESSION_COOKIE}="))), None);
    assert_eq!(session_token(&req("other=1")), None);
    assert!(Caller::Local.can(Permission::Accounts) && !Caller::Guest.can(Permission::Prefill));
    let player = Caller::Signed { name: "x".into(), role: Role::Player };
    assert_eq!(player.permissions(), [Permission::PrefillOwn]);
    assert_eq!(needs(Route::PrefillStart), Some(Permission::Prefill));
    assert_eq!(needs(Route::Status), None, "reading is open");
}
