//! Tests for `lanstash_steam_domain::cm::logon`.

use lanstash_steam_domain::cm::logon::{ANONYMOUS, LogonResponse, PROTOCOL_VERSION, anonymous_logon, heartbeat, hello};
use lanstash_steam_domain::cm::message::{CmError, Header, Message, emsg};
use lanstash_steam_domain::cm::proto::{Wire, Writer, fields};

#[test]
fn hello_and_logon_carry_the_protocol_version() {
    for (m, id) in [(hello(), emsg::CLIENT_HELLO), (anonymous_logon(), emsg::CLIENT_LOGON)] {
        let back = Message::decode(&m.encode()).unwrap();
        assert_eq!(back.emsg, id);
        let f: Vec<_> = fields(&back.body).map(Result::unwrap).collect();
        assert_eq!(f, [(1, Wire::Varint(u64::from(PROTOCOL_VERSION)))]);
    }
    assert_eq!(anonymous_logon().header.steamid, ANONYMOUS);
}

#[test]
fn a_heartbeat_goes_out_as_the_session() {
    let s = Header { steamid: 7, session: 9, ..Header::default() };
    let h = heartbeat(&s);
    assert_eq!((h.emsg, h.header.steamid, h.header.session), (emsg::CLIENT_HEART_BEAT, 7, 9));
}

#[test]
fn a_logon_response_reads_its_result_heartbeat_and_cell() {
    let body = Writer::new().varint(1, 1).varint(2, 9).varint(3, 9).varint(7, 148).string(8, "x").finish();
    assert_eq!(LogonResponse::decode(&body), Ok(LogonResponse { eresult: 1, heartbeat_seconds: 9, cell_id: 148 }));
    assert_eq!(LogonResponse::decode(&[0x08]), Err(CmError::Malformed("logon response")));
}
