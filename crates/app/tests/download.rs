//! Tests for `lanstash_app::download`.

use lanstash_app::download::*;
use lanstash_domain::extent::Span;
use lanstash_store::test_support::MemFile;
use std::sync::Arc;

fn content(span: Span) -> DlHead {
    DlHead { kind: HeadKind::Content { span, total: Some(span.end) }, headers: vec![] }
}

fn bytes(p: Piece) -> Vec<u8> {
    match p {
        Piece::Bytes(b) => b,
        other => panic!("expected bytes, got {other:?}"),
    }
}

#[test]
fn clients_read_as_bytes_arrive() {
    let d = Arc::new(Download::new(Some(Span::new(10, 20)), 1 << 20));
    let reader = {
        let d = Arc::clone(&d);
        std::thread::spawn(move || {
            let mut got = Vec::new();
            let mut pos = 10;
            loop {
                match d.next(pos, 3) {
                    Piece::Bytes(b) => {
                        pos += b.len() as u64;
                        got.extend(b);
                    }
                    Piece::End => return got,
                    other => panic!("{other:?}"),
                }
            }
        })
    };
    d.set_head(content(Span::new(10, 20)), Span::new(10, 20), false);
    d.push(b"01234");
    d.push(b"56789");
    d.finish(Ok(()));
    assert_eq!(reader.join().unwrap(), b"0123456789");
    assert!(matches!(d.next(20, 10), Piece::End));
    assert_eq!(d.wait_end(), Ok(10));
}

#[test]
fn the_last_byte_waits_for_the_check() {
    let d = Download::new(None, 1 << 20);
    d.set_head(content(Span::new(0, 4)), Span::new(0, 4), true);
    d.push(b"abcd");
    assert_eq!(bytes(d.next(0, 10)), b"abc", "final byte withheld");
    d.finish(Ok(()));
    assert_eq!(bytes(d.next(3, 10)), b"d");

    let bad = Download::new(None, 1 << 20);
    bad.set_head(content(Span::new(0, 4)), Span::new(0, 4), true);
    bad.push(b"abcd");
    bad.finish(Err("x-content-crc mismatch".into()));
    assert!(matches!(bad.next(0, 10), Piece::Failed(_)), "nothing of a failed checked body is served");
}

#[test]
fn a_failed_download_still_serves_what_arrived_then_fails() {
    let d = Download::new(Some(Span::new(0, 100)), 1 << 20);
    d.set_head(content(Span::new(0, 100)), Span::new(0, 100), false);
    d.push(b"0123");
    d.finish(Err("reset".into()));
    assert_eq!(bytes(d.next(0, 10)), b"0123");
    assert!(matches!(d.next(4, 10), Piece::Failed(_)));
    assert_eq!(d.wait_end(), Err("reset".into()));
}

#[test]
fn unknown_length_ends_where_the_bytes_end() {
    let d = Download::new(None, 1 << 20);
    let head = DlHead { kind: HeadKind::Content { span: Span::new(0, u64::MAX), total: None }, headers: vec![] };
    d.set_head(head, Span::new(0, u64::MAX), false);
    d.push(b"xyz");
    d.finish(Ok(()));
    assert_eq!(bytes(d.next(0, 10)), b"xyz");
    assert!(matches!(d.next(3, 10), Piece::End));
}

#[test]
fn laggards_read_back_from_storage() {
    let file = MemFile::default();
    let d = Download::new(None, 64 * 1024);
    d.set_head(content(Span::new(0, 300_000)), Span::new(0, 300_000), false);
    d.set_read_back(Arc::new(file.clone()), 1000);
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 253) as u8).collect();
    use lanstash_ports::storage::FileWrite;
    file.write_all_at(1000, &data).unwrap();
    for part in data.chunks(10_000) {
        d.push(part);
    }
    match d.next(0, 5000) {
        Piece::Storage { offset, len, .. } => assert_eq!((offset, len), (1000, 5000)),
        other => panic!("{other:?}"),
    }
    assert_eq!(bytes(d.next(299_000, 5000)), data[299_000..]);
}

#[test]
fn a_download_that_cannot_be_stored_keeps_a_bounded_amount_in_memory() {
    use lanstash_app::download::MEMORY_WINDOWS;
    let window = 64 * 1024;
    let total = (window * MEMORY_WINDOWS * 4) as u64;
    let d = Download::new(None, window);
    d.set_head(content(Span::new(0, total)), Span::new(0, total), false);
    let part = vec![7u8; 10_000];
    let mut sent = 0;
    while sent < total {
        d.push(&part);
        sent += part.len() as u64;
    }
    assert!(matches!(d.next(0, 10), Piece::Failed(_)), "the start left memory: a client there asks again");
    assert_eq!(bytes(d.next(total - 10, 10)), [7u8; 10], "the latest bytes are there");
}

#[test]
fn a_relayed_server_error_is_known_as_one_before_it_ends() {
    let relay = |status| DlHead { kind: HeadKind::Relay { status, content_length: Some(11) }, headers: vec![] };
    let gateway = Download::new(None, 1 << 20);
    assert!(!gateway.relays_server_error(), "no head yet");
    gateway.set_head(relay(502), Span::new(0, 11), false);
    assert!(gateway.relays_server_error() && !gateway.is_finished());
    let missing = Download::new(None, 1 << 20);
    missing.set_head(relay(404), Span::new(0, 11), false);
    assert!(!missing.relays_server_error(), "a 404 is the object's answer, not a passing one");
    let data = Download::new(None, 1 << 20);
    data.set_head(content(Span::new(0, 4)), Span::new(0, 4), false);
    assert!(!data.relays_server_error());
}

#[test]
fn a_failed_request_fails_waiting_clients() {
    let d = Download::new(None, 1 << 20);
    d.finish(Err("connection refused".into()));
    assert_eq!(d.wait_head(), Err("connection refused".into()));
    assert!(matches!(d.next(0, 1), Piece::Failed(_)));
    assert!(d.is_finished() && d.span().is_none());
}
