//! Tests for `lanstash_steam_app::connection`, against a scripted Steam.

mod support;

use std::sync::Mutex;

use lanstash_steam_app::connection::{CallError, Connection};
use lanstash_steam_domain::cm::message::{Header, Message, emsg};
use lanstash_steam_domain::cm::proto::Writer;
use support::{FakeDialer, answer, multi, steam};

#[test]
fn open_tries_the_listed_servers_in_order_and_says_hello() {
    let (link, sent) = steam(Box::new(|_| Vec::new()));
    let d = FakeDialer {
        servers: vec!["down.example:443", "up.example:27018"],
        down: vec!["down.example"],
        link: Mutex::new(Some(link)),
        dialed: Mutex::default(),
    };
    Connection::open(&d).unwrap();
    assert_eq!(*d.dialed.lock().unwrap(), ["down.example:443/cmsocket/", "up.example:27018/cmsocket/"]);
    assert_eq!(sent.lock().unwrap()[0].emsg, emsg::CLIENT_HELLO);
}

#[test]
fn open_fails_when_no_server_answers() {
    let d = FakeDialer {
        servers: vec!["a.example:1"],
        down: vec!["a.example"],
        link: Mutex::default(),
        dialed: Mutex::default(),
    };
    assert_eq!(Connection::open(&d).unwrap_err(), "a.example:1: refused");
}

#[test]
fn calls_are_matched_by_job_and_multis_unpacked() {
    let (link, sent) = steam(Box::new(|m| {
        let other = Message::new(5501, Header::default(), Vec::new()).encode();
        let wrong_job = answer(&Message::new(0, Header { job_source: 99, ..Header::default() }, Vec::new()), 1, vec![]);
        let old_style = 751u32.to_le_bytes().to_vec();
        vec![old_style, multi(&[other, wrong_job, answer(m, 1, vec![0x08, 0x2A])])]
    }));
    let mut c = Connection::over(link);
    let a = c.call("Some.Method#1", vec![]).unwrap();
    assert_eq!(a.body, [0x08, 0x2A]);
    let b = c.call("Some.Method#1", vec![]).unwrap();
    assert_eq!(b.header.job_target, 2, "job ids count up");
    assert_eq!(sent.lock().unwrap()[0].header.target_job_name.as_deref(), Some("Some.Method#1"));
}

#[test]
fn a_failed_call_names_the_result() {
    let (link, _) = steam(Box::new(|m| vec![answer(m, 84, vec![])]));
    let mut c = Connection::over(link);
    let e = c.call("X.Y#1", vec![]).unwrap_err();
    assert_eq!(e, CallError::Steam(84));
    assert_eq!(e.to_string(), "Steam: too many attempts; wait a while (84)");
}

#[test]
fn silence_and_being_logged_off_are_errors() {
    let (link, _) = steam(Box::new(|_| Vec::new()));
    let mut c = Connection::over(link);
    assert_eq!(c.call("X.Y#1", vec![]).unwrap_err(), CallError::Link("Steam did not answer in time".into()));
    let (link, _) = steam(Box::new(|_| {
        vec![Message::new(emsg::CLIENT_LOGGED_OFF, Header::default(), Writer::new().varint(1, 6).finish()).encode()]
    }));
    let mut c = Connection::over(link);
    c.send(&Message::new(1, Header::default(), vec![])).unwrap();
    assert_eq!(c.receive().unwrap_err(), "Steam logged this connection off (an unexpected result (6))");
}

#[test]
fn messages_go_out_as_the_session_once_there_is_one() {
    let (link, sent) = steam(Box::new(|_| Vec::new()));
    let mut c = Connection::over(link);
    c.session = Header { steamid: 76_561_197_960_287_930, session: 12, ..Header::default() };
    c.send(&Message::new(703, Header::default(), vec![])).unwrap();
    c.send(&Message::new(703, Header { steamid: 5, ..Header::default() }, vec![])).unwrap();
    let s = sent.lock().unwrap();
    assert_eq!((s[0].header.steamid, s[0].header.session), (76_561_197_960_287_930, 12));
    assert_eq!(s[1].header.steamid, 5, "a message that names its sender keeps it");
}

#[test]
fn the_license_list_is_kept_and_requests_get_their_replies() {
    use lanstash_steam_domain::cm::pics::emsg as pics;
    let (link, _) = steam(Box::new(|m| {
        let license = Writer::new().varint(1, 37).varint(17, 9).finish();
        let list =
            Message::new(pics::CLIENT_LICENSE_LIST, Header::default(), Writer::new().bytes(2, &license).finish());
        let reply = |part: u64| {
            let h = Header { job_target: m.header.job_source, ..Header::default() };
            Message::new(pics::PICS_PRODUCT_INFO_RESPONSE, h, Writer::new().varint(9, part).finish()).encode()
        };
        vec![multi(&[list.encode(), reply(1)]), reply(2)]
    }));
    let mut c = Connection::over(link);
    let job = c.request(pics::PICS_PRODUCT_INFO_REQUEST, vec![]).unwrap();
    assert_eq!(c.reply(job, pics::PICS_PRODUCT_INFO_RESPONSE).unwrap().body, Writer::new().varint(9, 1).finish());
    assert_eq!(c.reply(job, pics::PICS_PRODUCT_INFO_RESPONSE).unwrap().body, Writer::new().varint(9, 2).finish());
    let l = c.licenses().unwrap();
    assert_eq!((l.len(), l[0].package_id, l[0].access_token), (1, 37, 9));
}

#[test]
fn calls_once_logged_on_go_as_the_session() {
    let (link, sent) = steam(Box::new(|m| vec![answer(m, 1, vec![])]));
    let mut c = Connection::over(link);
    c.call("A.B#1", vec![]).unwrap();
    c.session = Header { steamid: 76_561_197_960_287_930, session: 3, ..Header::default() };
    c.call("A.B#1", vec![]).unwrap();
    let s = sent.lock().unwrap();
    assert_eq!(
        (s[0].emsg, s[1].emsg),
        (emsg::SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED, emsg::SERVICE_METHOD_CALL_FROM_CLIENT)
    );
}
