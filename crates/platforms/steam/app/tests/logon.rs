//! Tests for `lanstash_steam_app::logon`, against a scripted Steam.

mod support;

use lanstash_ports::secrets::Secrets;
use lanstash_steam_app::connection::Connection;
use lanstash_steam_app::login::token_name;
use lanstash_steam_app::logon::{LoggedOn, beat, logon};
use lanstash_steam_domain::cm::message::{Header, Message, emsg};
use lanstash_steam_domain::cm::proto::{Wire, Writer, fields};
use support::{Memory, multi, steam};

const STEAMID: u64 = 76_561_197_960_287_930;

fn response(eresult: u64) -> Vec<u8> {
    let header = Header { steamid: STEAMID, session: -7, ..Header::default() };
    let body = Writer::new().varint(1, eresult).varint(3, 9).varint(7, 127).finish();
    Message::new(emsg::CLIENT_LOG_ON_RESPONSE, header, body).encode()
}

fn kept() -> Memory {
    let s = Memory::default();
    s.save(&token_name("someone"), "REFRESH\n").unwrap();
    s
}

#[test]
fn the_kept_token_logs_on_and_the_session_is_used_after() {
    let (link, sent) =
        steam(Box::new(|m| if m.emsg == emsg::CLIENT_LOGON { vec![multi(&[response(1)])] } else { vec![] }));
    let mut c = Connection::over(link);
    let on = logon(&mut c, &kept(), "someone").unwrap();
    assert_eq!(on, LoggedOn { account: "someone".into(), steamid: STEAMID, cell_id: 127, heartbeat_seconds: 9 });
    beat(&mut c).unwrap();
    let sent = sent.lock().unwrap();
    let f: Vec<_> = fields(&sent[0].body).map(Result::unwrap).collect();
    assert!(f.contains(&(108, Wire::Bytes(b"REFRESH"))), "the token is sent trimmed");
    assert_eq!(sent[1].emsg, emsg::CLIENT_HEART_BEAT);
    assert_eq!((sent[1].header.steamid, sent[1].header.session), (STEAMID, -7));
}

#[test]
fn without_a_token_it_says_to_sign_in() {
    let (link, sent) = steam(Box::new(|_| vec![]));
    let mut c = Connection::over(link);
    assert_eq!(
        logon(&mut c, &Memory::default(), "someone").unwrap_err(),
        "someone is not signed in to Steam: sign in from the web UI"
    );
    assert!(sent.lock().unwrap().is_empty());
}

#[test]
fn a_refused_token_says_to_log_in_again() {
    let (link, _) = steam(Box::new(|_| vec![response(5)]));
    let mut c = Connection::over(link);
    let e = logon(&mut c, &kept(), "someone").unwrap_err();
    assert_eq!(e, "Steam refused the kept token: wrong account name or password (5); sign in again");
}

#[test]
fn a_passing_failure_is_not_a_refused_token() {
    for code in [20, 48, 2] {
        let (link, _) = steam(Box::new(move |_| vec![response(code)]));
        let mut c = Connection::over(link);
        let e = logon(&mut c, &kept(), "someone").unwrap_err();
        assert!(e.starts_with("Steam could not log on now: "), "{code}: {e}");
    }
}
