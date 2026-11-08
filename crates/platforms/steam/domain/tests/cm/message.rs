//! Tests for `lanstash_steam_domain::cm::message`.

use lanstash_steam_domain::cm::message::{CmError, Header, Message, NO_JOB, emsg, unpack_multi};
use lanstash_steam_domain::cm::proto::Writer;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn a_message_round_trips_with_its_header() {
    let header = Header {
        steamid: 0x0110_0001_0000_0001,
        session: -5,
        job_source: 42,
        target_job_name: Some("Authentication.GetPasswordRSAPublicKey#1".into()),
        eresult: 84,
        error_message: Some("slow down".into()),
        ..Header::default()
    };
    let m = Message::new(emsg::SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED, header, vec![1, 2, 3]);
    let bytes = m.encode();
    assert_eq!(&bytes[..4], (9804u32 | 0x8000_0000).to_le_bytes());
    let back = Message::decode(&bytes).unwrap();
    assert_eq!(back, m);
    assert_eq!(back.header.job_target, NO_JOB);
}

#[test]
fn old_style_and_short_messages_are_refused() {
    assert_eq!(Message::decode(&751u32.to_le_bytes()), Err(CmError::NotProtobuf(751)));
    assert_eq!(Message::decode(&[1, 0, 0, 0x80, 9, 0, 0, 0, 0]), Err(CmError::Truncated));
    assert_eq!(Message::decode(&[1, 0]), Err(CmError::Truncated));
}

#[test]
fn a_multi_unpacks_plain_and_gzipped() {
    let inner = [b"first message".as_slice(), b"second"];
    let mut plain = Vec::new();
    for m in inner {
        plain.extend_from_slice(&(m.len() as u32).to_le_bytes());
        plain.extend_from_slice(m);
    }
    let body = Writer::new().bytes(2, &plain).finish();
    assert_eq!(unpack_multi(&body, 1 << 20).unwrap(), inner);
    // Python's gzip.compress of the same bytes.
    let gz = unhex("1f8b08000000000002ffe365606048cb2c2a2e51c84d2d2e4e4c4f65030a14a726e7e7a50000e8e76bb61b000000");
    let body = Writer::new().varint(1, plain.len() as u64).bytes(2, &gz).finish();
    assert_eq!(unpack_multi(&body, 1 << 20).unwrap(), inner);
    let wrong = Writer::new().varint(1, 99).bytes(2, &gz).finish();
    assert_eq!(unpack_multi(&wrong, 1 << 20), Err(CmError::Malformed("multi size")));
    let cut = Writer::new().bytes(2, &plain[..plain.len() - 1]).finish();
    assert_eq!(unpack_multi(&cut, 1 << 20), Err(CmError::Truncated));
}
