//! Tests for `lanstash_steam_domain::cm::auth`: requests read back field by field,
//! responses built here with the field numbers of Steam's definitions.

use lanstash_steam_domain::cm::auth::{
    Guard, INDIVIDUAL, PollStatus, QrSession, RsaKey, Session, Tokens, begin_qr, begin_session, call, guard_code,
    method, poll, rsa_key_request, token_logon,
};
use lanstash_steam_domain::cm::message::{CmError, Message, emsg};
use lanstash_steam_domain::cm::proto::{Wire, Writer, fields};

fn read(body: &[u8]) -> Vec<(u32, Wire<'_>)> {
    fields(body).map(Result::unwrap).collect()
}

#[test]
fn a_call_names_its_method_and_job() {
    let m = call(method::GET_PASSWORD_RSA_PUBLIC_KEY, 7, rsa_key_request("someone"));
    let back = Message::decode(&m.encode()).unwrap();
    assert_eq!(back.emsg, emsg::SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED);
    assert_eq!(back.header.job_source, 7);
    assert_eq!(back.header.target_job_name.as_deref(), Some("Authentication.GetPasswordRSAPublicKey#1"));
    assert_eq!(read(&back.body), [(1, Wire::Bytes(b"someone"))]);
}

#[test]
fn the_rsa_key_reads_and_needs_a_modulus() {
    let body = Writer::new().string(1, "c0ffee").string(2, "010001").varint(3, 123_456).finish();
    let k = RsaKey::decode(&body).unwrap();
    assert_eq!((k.modulus_hex.as_str(), k.exponent_hex.as_str(), k.timestamp), ("c0ffee", "010001", 123_456));
    let none = Writer::new().string(2, "010001").finish();
    assert_eq!(RsaKey::decode(&none), Err(CmError::Malformed("rsa key without a modulus")));
}

#[test]
fn a_session_request_carries_the_account_and_device() {
    let body = begin_session("lanstash", "someone", "BASE64==", 99);
    let f = read(&body);
    assert_eq!(f[0], (1, Wire::Bytes(b"lanstash")));
    assert_eq!(f[1], (2, Wire::Bytes(b"someone")));
    assert_eq!(f[2], (3, Wire::Bytes(b"BASE64==")));
    assert_eq!(f[3], (4, Wire::Varint(99)));
    assert_eq!(f[4], (5, Wire::Varint(1)));
    assert_eq!(f[7], (8, Wire::Bytes(b"Client")));
    let details = read(f[8].1.bytes().unwrap());
    assert_eq!(details[0], (1, Wire::Bytes(b"lanstash")));
    assert_eq!(details[2].1.int().map(|v| v as i32), Some(-203));
}

fn session_body() -> Vec<u8> {
    let email = Writer::new().varint(1, 2).string(2, "ex***@example.com").finish();
    let device = Writer::new().varint(1, 4).finish();
    Writer::new()
        .varint(1, 555)
        .bytes(2, &[9, 8, 7])
        .fixed32(3, 5.0f32.to_bits())
        .bytes(4, &email)
        .bytes(4, &device)
        .varint(5, 76_561_197_960_287_930)
        .finish()
}

#[test]
fn a_session_lists_its_confirmations() {
    let s = Session::decode(&session_body()).unwrap();
    assert_eq!(s.client_id, 555);
    assert_eq!(s.request_id, [9, 8, 7]);
    assert!((s.interval - 5.0).abs() < f32::EPSILON);
    assert_eq!(s.confirmations, [Guard::EmailCode, Guard::DeviceConfirmation]);
    assert_eq!(s.steamid, 76_561_197_960_287_930);
    let empty = Writer::new().bytes(4, &[]).finish();
    assert_eq!(Session::decode(&empty), Err(CmError::Malformed("allowed confirmation")));
}

#[test]
fn a_code_and_a_poll_name_the_session() {
    let s = Session::decode(&session_body()).unwrap();
    let code = guard_code(&s, "AB12C", Guard::EmailCode);
    let f = read(&code);
    assert_eq!(
        f,
        [
            (1, Wire::Varint(555)),
            (2, Wire::Fixed64(76_561_197_960_287_930)),
            (3, Wire::Bytes(b"AB12C")),
            (4, Wire::Varint(2))
        ]
    );
    assert_eq!(read(&poll(&s)), [(1, Wire::Varint(555)), (2, Wire::Bytes(&[9, 8, 7]))]);
    assert_eq!(Guard::Other(9).number(), 9);
}

#[test]
fn tokens_come_once_confirmed_and_never_print() {
    assert_eq!(Tokens::decode(&Writer::new().varint(5, 0).finish()), Ok(None));
    let body = Writer::new().string(3, "REFRESH").string(4, "ACCESS").string(6, "someone").finish();
    let t = Tokens::decode(&body).unwrap().unwrap();
    assert_eq!((t.refresh_token.as_str(), t.access_token.as_str()), ("REFRESH", "ACCESS"));
    let shown = format!("{t:?}");
    assert!(shown.contains("someone") && !shown.contains("REFRESH") && !shown.contains("ACCESS"));
}

#[test]
fn a_token_logon_is_an_individual_account() {
    let m = token_logon("someone", "REFRESH");
    assert_eq!((m.emsg, m.header.steamid), (emsg::CLIENT_LOGON, INDIVIDUAL));
    let f = read(&m.body);
    assert!(f.contains(&(50, Wire::Bytes(b"someone"))));
    assert!(f.contains(&(108, Wire::Bytes(b"REFRESH"))));
    assert!(f.contains(&(8, Wire::Varint(1))));
}

/// The shape of Steam's answer, with made-up values.
fn qr_body() -> Vec<u8> {
    Writer::new()
        .varint(1, 18_400_974_508_802_468_659)
        .string(2, "https://s.team/q/1/18400974508802468659")
        .bytes(3, &[7; 16])
        .fixed32(4, 0x40A0_0000) // 5.0
        .bytes(5, &[8, 4])
        .bytes(5, &[8, 3])
        .varint(6, 1)
        .finish()
}

#[test]
fn a_qr_session_carries_its_link() {
    let q = QrSession::decode(&qr_body()).unwrap();
    assert_eq!(q.challenge_url, "https://s.team/q/1/18400974508802468659");
    assert_eq!(q.session.client_id, 18_400_974_508_802_468_659);
    assert_eq!(q.session.request_id, [7; 16]);
    assert!((q.session.interval - 5.0).abs() < f32::EPSILON);
    assert_eq!(q.session.confirmations, [Guard::DeviceConfirmation, Guard::DeviceCode]);
    let no_link = Writer::new().varint(1, 5).finish();
    assert_eq!(QrSession::decode(&no_link), Err(CmError::Malformed("qr session without a link")));
}

#[test]
fn a_qr_request_names_the_device() {
    let b = begin_qr("lanstash");
    let f = read(&b);
    assert_eq!(f[0], (1, Wire::Bytes(b"lanstash")));
    assert_eq!(f[1], (2, Wire::Varint(1)));
    assert_eq!(f[3], (4, Wire::Bytes(b"Client")));
}

#[test]
fn a_poll_reports_a_new_link_a_scan_and_the_tokens() {
    let rotated = Writer::new().varint(1, 9).string(2, "https://s.team/q/1/9").finish();
    let p = PollStatus::decode(&rotated).unwrap();
    assert_eq!(
        (p.new_client_id, p.new_challenge_url.as_deref(), p.tokens),
        (Some(9), Some("https://s.team/q/1/9"), None)
    );
    let scanned = Writer::new().bool(5, true).finish();
    assert!(PollStatus::decode(&scanned).unwrap().had_remote_interaction);
    let done = Writer::new().string(3, "REFRESH").string(6, "someone").finish();
    assert_eq!(PollStatus::decode(&done).unwrap().tokens.unwrap().account_name, "someone");
    assert_eq!(PollStatus::decode(&[]).unwrap(), PollStatus::default());
}
