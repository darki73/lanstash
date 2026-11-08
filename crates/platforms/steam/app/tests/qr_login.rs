//! Tests for `lanstash_steam_app::qr_login`, against a scripted Steam.

mod support;

use lanstash_steam_app::connection::Connection;
use lanstash_steam_app::login::{accounts, token};
use lanstash_steam_app::qr_login::{QrLogin, Step};
use lanstash_steam_domain::cm::auth::method;
use lanstash_steam_domain::cm::proto::{Writer, fields};
use support::{Memory, Script, answer, steam};

pub fn qr_session() -> Vec<u8> {
    Writer::new()
        .varint(1, 41)
        .string(2, "https://s.team/q/1/41")
        .bytes(3, &[7; 16])
        .fixed32(4, 0x40A0_0000)
        .bytes(5, &[8, 4])
        .finish()
}

/// Rotates the code on the first poll, reports a scan on the second, approves on the third.
fn script() -> Script {
    let mut polls = 0;
    Box::new(move |m| match m.header.target_job_name.as_deref() {
        Some(method::BEGIN_AUTH_SESSION_VIA_QR) => vec![answer(m, 1, qr_session())],
        Some(method::POLL_AUTH_SESSION_STATUS) => {
            polls += 1;
            let body = match polls {
                1 => Writer::new().varint(1, 42).string(2, "https://s.team/q/1/42").finish(),
                2 => Writer::new().bool(5, true).finish(),
                _ => Writer::new().string(3, "REFRESH").string(6, "someone").finish(),
            };
            vec![answer(m, 1, body)]
        }
        _ => Vec::new(),
    })
}

#[test]
fn a_poll_interval_that_is_not_a_number_waits_five_seconds() {
    let nan = Writer::new().varint(1, 41).string(2, "u").bytes(3, &[7; 16]).fixed32(4, 0x7FC0_0000).finish();
    let (link, _) = steam(Box::new(move |m| match m.header.target_job_name.as_deref() {
        Some(method::BEGIN_AUTH_SESSION_VIA_QR) => vec![answer(m, 1, nan.clone())],
        _ => Vec::new(),
    }));
    let q = QrLogin::start(&mut Connection::over(link), "lanstash").unwrap();
    assert_eq!(q.interval_ms(), 5000);
}

#[test]
fn a_scanned_and_approved_code_keeps_the_token() {
    let (link, sent) = steam(script());
    let mut c = Connection::over(link);
    let secrets = Memory::default();
    let mut q = QrLogin::start(&mut c, "lanstash").unwrap();
    assert_eq!((q.url.as_str(), q.scanned, q.interval_ms()), ("https://s.team/q/1/41", false, 5000));
    assert_eq!(q.poll(&mut c, &secrets).unwrap(), Step::Waiting);
    assert_eq!(q.url, "https://s.team/q/1/42", "the rotated code");
    assert_eq!(q.poll(&mut c, &secrets).unwrap(), Step::Waiting);
    assert!(q.scanned);
    assert_eq!(q.poll(&mut c, &secrets).unwrap(), Step::Done("someone".into()));
    assert_eq!(token(&secrets, "someone").unwrap().as_deref(), Some("REFRESH"));
    assert_eq!(accounts(&secrets).unwrap(), ["someone"]);
    // After the rotation, polls name the new client id.
    let sent = sent.lock().unwrap();
    let last =
        sent.iter().rev().find(|m| m.header.target_job_name.as_deref() == Some(method::POLL_AUTH_SESSION_STATUS));
    let id = fields(&last.unwrap().body).filter_map(Result::ok).find(|(n, _)| *n == 1).and_then(|(_, v)| v.int());
    assert_eq!(id, Some(42));
}

#[test]
fn an_expired_code_is_an_error() {
    let (link, _) = steam(Box::new(|m| match m.header.target_job_name.as_deref() {
        Some(method::BEGIN_AUTH_SESSION_VIA_QR) => vec![answer(m, 1, qr_session())],
        _ => vec![answer(m, 27, vec![])],
    }));
    let mut c = Connection::over(link);
    let mut q = QrLogin::start(&mut c, "lanstash").unwrap();
    assert_eq!(q.poll(&mut c, &Memory::default()).unwrap_err(), "Steam: expired (27)");
}
