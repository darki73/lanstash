//! Tests for `lanstash_steam_domain::cm::proto`.

use lanstash_steam_domain::cm::proto::{Wire, Writer, fields, put_varint, read_varint};

#[test]
fn varints_round_trip_at_their_edges() {
    for v in [0, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
        let mut b = Vec::new();
        put_varint(v, &mut b);
        assert_eq!(read_varint(&b, 0), Some((v, b.len())));
    }
    let mut b = Vec::new();
    put_varint(300, &mut b);
    assert_eq!(b, [0xAC, 0x02]); // the protobuf guide's example
    assert_eq!(read_varint(&[0x80], 0), None);
}

#[test]
fn a_written_message_reads_back_field_by_field() {
    let m = Writer::new()
        .varint(1, 150)
        .fixed64(2, 0x0110_0001_0000_0001)
        .string(3, "testing")
        .fixed32(4, 7)
        .bool(5, true)
        .varint(6, -1i64 as u64)
        .finish();
    assert_eq!(&m[..3], [0x08, 0x96, 0x01]); // field 1 = 150, as in the protobuf guide
    let got: Vec<_> = fields(&m).map(Result::unwrap).collect();
    assert_eq!(
        got,
        [
            (1, Wire::Varint(150)),
            (2, Wire::Fixed64(0x0110_0001_0000_0001)),
            (3, Wire::Bytes(b"testing")),
            (4, Wire::Fixed32(7)),
            (5, Wire::Varint(1)),
            (6, Wire::Varint(u64::MAX)),
        ]
    );
    assert_eq!(got[2].1.str(), Some("testing"));
    assert_eq!(got[5].1.int().map(|v| v as i32), Some(-1));
}

#[test]
fn a_cut_message_ends_with_one_error() {
    let m = Writer::new().varint(1, 5).string(2, "abcdef").finish();
    let got: Vec<_> = fields(&m[..m.len() - 2]).collect();
    assert_eq!(got, [Ok((1, Wire::Varint(5))), Err(())]);
    // Wire types 3 and 4 (groups) are not supported.
    assert_eq!(fields(&[0x0B]).collect::<Vec<_>>(), [Err(())]);
}
