//! Tests for `lanstash_steam_app::raw`: Steam's answers kept between runs.

use lanstash_steam_app::raw::{Raw, decode_raw, decode_raws, encode_raw, encode_raws};

fn one() -> Raw {
    Raw {
        owned: vec![101, 201],
        apps: vec![
            (100, br#""appinfo" { "common" { "name" "Some Game" "type" "Game" } }"#.to_vec()),
            (7, b"not key-values".to_vec()),
        ],
    }
}

#[test]
fn one_accounts_answers_are_kept_whole() {
    let raw = one();
    let b = encode_raw(&raw, 1_234);
    assert_eq!(decode_raw(&b), Some((raw, 1_234)));
    assert_eq!(decode_raw(&b[..b.len() - 1]), None, "cut short");
    assert_eq!(decode_raw(b"LSL2"), None, "another kind");
}

#[test]
fn several_accounts_answers_are_kept_and_one_kept_before_is_the_first_accounts() {
    let raws = vec![
        ("player_one".to_string(), one()),
        ("player_two".to_string(), Raw { owned: vec![5], apps: vec![(9, b"x".to_vec())] }),
    ];
    let b = encode_raws(&raws, 77);
    assert_eq!(decode_raws(&b, None), Some((raws.clone(), 77)));
    assert_eq!(decode_raws(&b[..b.len() - 1], None), None, "cut short");
    let old = encode_raw(&raws[0].1, 5);
    assert_eq!(decode_raws(&old, Some("player_one")), Some((vec![raws[0].clone()], 5)));
    assert_eq!(decode_raws(&old, None), None, "nobody to give it to");
    assert_eq!(decode_raws(b"nope", None), None);
}

#[test]
fn a_length_the_blob_does_not_hold_is_refused() {
    let mut b = encode_raws(&[("a".to_string(), one())], 1);
    // Magic, time, count, the name's length and name: then the answers' length.
    b[21..25].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode_raws(&b, None), None);
}
