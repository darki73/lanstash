//! Tests for `lanstash_steam_app::account`: the QR sign-in on its thread,
//! against a scripted Steam; a short real pause lets the test see each state.

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lanstash_ports::account::Account;
use lanstash_ports::clock::Pause;
use lanstash_ports::secrets::Secrets;
use lanstash_steam_app::account::{SteamAccount, Wiring};
use lanstash_steam_app::login::{ACCOUNT, REFRESH_TOKEN, token};
use lanstash_steam_domain::cm::auth::method;
use lanstash_steam_domain::cm::proto::Writer;
use support::{FakeDialer, Memory, Script, Sent, answer, steam};

/// Each 250 ms step of a pause takes 1 ms here: one 5 s poll interval is 20 ms.
struct Quick;

impl Pause for Quick {
    fn pause_ms(&self, _ms: u64) {
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn qr_session() -> Vec<u8> {
    Writer::new().varint(1, 41).string(2, "https://s.team/q/1/41").bytes(3, &[7; 16]).fixed32(4, 0x40A0_0000).finish()
}

/// Approves after `pending` polls (`u32::MAX`: never).
fn script(pending: u32) -> Script {
    let mut polls = 0;
    Box::new(move |m| match m.header.target_job_name.as_deref() {
        Some(method::BEGIN_AUTH_SESSION_VIA_QR) => vec![answer(m, 1, qr_session())],
        Some(method::POLL_AUTH_SESSION_STATUS) => {
            polls += 1;
            let body = if polls > pending {
                Writer::new().string(3, "REFRESH").string(6, "someone").finish()
            } else {
                Writer::new().bool(5, polls > 1).finish()
            };
            vec![answer(m, 1, body)]
        }
        _ => Vec::new(),
    })
}

fn account(script: Script) -> (SteamAccount, Arc<Memory>, Sent) {
    let (link, sent) = steam(script);
    let dialer = FakeDialer {
        servers: vec!["cm.example:443"],
        down: vec![],
        link: Mutex::new(Some(link)),
        dialed: Mutex::default(),
    };
    let secrets = Arc::new(Memory::default());
    let wiring = Wiring {
        dialer: Arc::new(dialer),
        secrets: secrets.clone(),
        pause: Arc::new(Quick),
        device: "lanstash".into(),
        refused: Arc::new(|_: &str| false),
        signed_out: Arc::new(|_: &str| {}),
        logons: Arc::new(|| 3),
    };
    (SteamAccount::new(wiring), secrets, sent)
}

/// Waits (at most 5 s) until the view contains `part`.
fn until(a: &SteamAccount, part: &str) -> String {
    let start = Instant::now();
    loop {
        let v = a.view();
        if v.contains(part) {
            return v;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "never {part:?}; last {v}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_qr_sign_in_shows_its_code_then_the_account() {
    let (a, secrets, _) = account(script(5));
    assert_eq!(
        a.view(),
        "{\"accounts\":[],\"account\":null,\"sign_in\":{\"state\":\"idle\",\"scanned\":false,\"error\":null},\"logons\":3}\n"
    );
    a.start_sign_in().unwrap();
    until(&a, "\"state\":\"waiting\"");
    assert!(a.qr_svg().unwrap().starts_with("<svg"));
    until(&a, "\"scanned\":true");
    let v = until(&a, "\"account\":\"someone\"");
    assert!(v.contains("\"state\":\"idle\""), "{v}");
    assert!(a.qr_svg().is_none());
    assert_eq!(token(secrets.as_ref(), "someone").unwrap().as_deref(), Some("REFRESH"));
    assert!(v.contains("\"accounts\":[{\"name\":\"someone\",\"refused\":false}]"), "{v}");
}

#[test]
fn a_cancelled_sign_in_stops_polling() {
    let (a, _, sent) = account(script(u32::MAX));
    a.start_sign_in().unwrap();
    until(&a, "\"state\":\"waiting\"");
    a.cancel_sign_in();
    assert!(a.view().contains("\"state\":\"idle\""));
    std::thread::sleep(Duration::from_millis(100));
    let n = sent.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(sent.lock().unwrap().len(), n, "no more polls after the cancel");
    assert!(a.qr_svg().is_none());
}

#[test]
fn a_refused_start_is_shown_as_failed() {
    let (a, _, _) = account(Box::new(|m| vec![answer(m, 84, vec![])]));
    a.start_sign_in().unwrap();
    let v = until(&a, "\"state\":\"failed\"");
    assert!(v.contains("too many attempts"), "{v}");
}

#[test]
fn signing_out_forgets_that_accounts_token_and_its_session() {
    let (a, secrets, _) = account(script(0));
    secrets.save(REFRESH_TOKEN, "x").unwrap();
    secrets.save(ACCOUNT, "someone").unwrap();
    let v = a.view();
    assert!(v.starts_with("{\"accounts\":[{\"name\":\"someone\",\"refused\":false}],\"account\":\"someone\""), "{v}");
    assert!(a.sign_out("nobody").is_err());
    a.sign_out("someone").unwrap();
    assert!(a.view().starts_with("{\"accounts\":[],\"account\":null"));
    assert!(token(secrets.as_ref(), "someone").unwrap().is_none());
}

#[test]
fn each_account_says_whether_its_sign_in_is_refused() {
    let secrets = Arc::new(Memory::default());
    secrets.save(REFRESH_TOKEN, "x").unwrap();
    secrets.save(ACCOUNT, "someone").unwrap();
    secrets.save("steam-token-another", "y").unwrap();
    secrets.save("steam-accounts", "someone\nanother\n").unwrap();
    let (link, _) = steam(Box::new(|_| vec![]));
    let dialer = FakeDialer { servers: vec![], down: vec![], link: Mutex::new(Some(link)), dialed: Mutex::default() };
    let dropped = Arc::new(Mutex::new(Vec::<String>::new()));
    let gone = Arc::clone(&dropped);
    let wiring = Wiring {
        dialer: Arc::new(dialer),
        secrets: secrets.clone(),
        pause: Arc::new(Quick),
        device: "lanstash".into(),
        refused: Arc::new(|account: &str| account == "another"),
        signed_out: Arc::new(move |account: &str| gone.lock().unwrap().push(account.to_string())),
        logons: Arc::new(|| 3),
    };
    let a = SteamAccount::new(wiring);
    let v = a.view();
    assert!(
        v.starts_with(
            "{\"accounts\":[{\"name\":\"someone\",\"refused\":false},{\"name\":\"another\",\"refused\":true}],\"account\":\"someone\",\"sign_in\":{\"state\":\"idle\""
        ),
        "{v}"
    );
    a.sign_out("another").unwrap();
    assert_eq!(*dropped.lock().unwrap(), ["another"], "its session is dropped");
}

/// Hands out one scripted link per connection, in order.
struct Links(Mutex<Vec<Box<dyn lanstash_ports::link::Link>>>);

impl lanstash_ports::link::Dialer for Links {
    fn dial(&self, _h: &str, _p: u16, _path: &str) -> Result<Box<dyn lanstash_ports::link::Link>, String> {
        let mut links = self.0.lock().unwrap();
        if links.is_empty() { Err("no more links".into()) } else { Ok(links.remove(0)) }
    }
    fn get(&self, _h: &str, _path: &str) -> Result<Vec<u8>, String> {
        Ok(b"{\"response\":{\"serverlist\":[{\"endpoint\":\"cm.example:443\",\"type\":\"websockets\"}],\"success\":true}}".to_vec())
    }
}

fn account_over(links: Vec<Box<dyn lanstash_ports::link::Link>>) -> (SteamAccount, Arc<Memory>) {
    let secrets = Arc::new(Memory::default());
    let wiring = Wiring {
        dialer: Arc::new(Links(Mutex::new(links))),
        secrets: secrets.clone(),
        pause: Arc::new(Quick),
        device: "lanstash".into(),
        refused: Arc::new(|_: &str| false),
        signed_out: Arc::new(|_: &str| {}),
        logons: Arc::new(|| 0),
    };
    (SteamAccount::new(wiring), secrets)
}

/// Gives a code, then goes quiet: the connection breaks at the first poll.
fn quiet_after_the_code() -> Script {
    Box::new(|m| match m.header.target_job_name.as_deref() {
        Some(method::BEGIN_AUTH_SESSION_VIA_QR) => vec![answer(m, 1, qr_session())],
        _ => Vec::new(),
    })
}

#[test]
fn a_code_whose_connection_broke_unscanned_is_replaced_by_a_new_one() {
    let (first, _) = steam(quiet_after_the_code());
    let (second, _) = steam(script(0));
    let (a, secrets) = account_over(vec![first, second]);
    a.start_sign_in().unwrap();
    let v = until(&a, "\"accounts\":[{\"name\":\"someone\"");
    assert!(v.contains("\"state\":\"idle\""), "{v}");
    assert_eq!(token(secrets.as_ref(), "someone").unwrap().as_deref(), Some("REFRESH"));
}

#[test]
fn codes_are_replaced_a_few_times_only() {
    let links = (0..5).map(|_| steam(quiet_after_the_code()).0).collect();
    let (a, _) = account_over(links);
    a.start_sign_in().unwrap();
    let v = until(&a, "\"state\":\"failed\"");
    assert!(!v.contains("no more links"), "gave up before using every link: {v}");
}
