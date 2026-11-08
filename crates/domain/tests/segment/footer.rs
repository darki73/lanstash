//! Tests for `lanstash_domain::segment::footer`.

use lanstash_domain::segment::footer::*;

#[test]
fn fnv1a_known_values() {
    // Published FNV-1a 64 test vectors.
    assert_eq!(key_hash(b""), 0xCBF2_9CE4_8422_2325);
    assert_eq!(key_hash(b"a"), 0xAF63_DC4C_8601_EC8C);
    assert_eq!(key_hash(b"foobar"), 0x8594_4171_F739_67E8);
}

#[test]
fn footer_round_trip_and_sorted() {
    let mut entries = vec![
        FooterEntry { key_hash: 9, offset: 4096, body_len: 10, flags: 0, header_len: 0 },
        FooterEntry { key_hash: 3, offset: 8192, body_len: 20, flags: 1, header_len: 136 },
        FooterEntry { key_hash: 3, offset: 4096 * 5, body_len: 30, flags: 0, header_len: 0 },
    ];
    let enc = encode_footer(&mut entries);
    assert_eq!(enc.len(), 3 * FOOTER_ENTRY_LEN);
    let dec = decode_footer(&enc).unwrap();
    assert_eq!(dec, entries);
    assert_eq!(dec[0].key_hash, 3);
    assert_eq!(find_entries(&dec, 3).len(), 2);
    assert_eq!(find_entries(&dec, 9).len(), 1);
    assert!(find_entries(&dec, 4).is_empty());
}

#[test]
fn footer_rejects_bad_length_and_order() {
    assert!(decode_footer(&[0u8; 31]).is_err());
    let mut e = vec![
        FooterEntry { key_hash: 1, offset: 0, body_len: 0, flags: 0, header_len: 0 },
        FooterEntry { key_hash: 2, offset: 0, body_len: 0, flags: 0, header_len: 0 },
    ];
    let mut enc = encode_footer(&mut e);
    enc.swap(0, 32); // swap first byte of each hash → 2 before 1
    assert!(decode_footer(&enc).is_err());
}

#[test]
fn trailer_round_trip_and_corruption() {
    let t = Trailer { footer_offset: 1 << 30, entry_count: 1000, footer_crc: 0x1234_5678 };
    let enc = t.encode();
    assert_eq!(Trailer::decode(&enc).unwrap(), t);
    for i in 0..TRAILER_LEN {
        let mut bad = enc;
        bad[i] ^= 0x10;
        assert!(Trailer::decode(&bad).is_err(), "flip at byte {i} not detected");
    }
}
