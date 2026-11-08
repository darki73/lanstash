//! Tests for `lanstash_steam_domain::kv`, on KeyValues shaped like PICS's
//! answers (written here, not copied from Steam).

use lanstash_steam_domain::kv::{Kv, KvError, parse_binary, parse_text};

#[test]
fn text_sections_values_escapes_and_comments() {
    let text = b"\"appinfo\"\n{\n\t\"appid\"\t\t\"7\"\n\t// a comment\n\t\"common\"\n\t{\n\t\t\"name\"\t\t\"A \\\"quoted\\\" game\"\n\t\tbare\tword\n\t}\n}\n\0";
    let kv = parse_text(text).unwrap();
    let app = kv.get("APPINFO").unwrap();
    assert_eq!(app.get("appid").and_then(Kv::int), Some(7));
    assert_eq!(app.at(&["common", "name"]).and_then(Kv::text).as_deref(), Some("A \"quoted\" game"));
    assert_eq!(app.at(&["common", "bare"]).and_then(Kv::text).as_deref(), Some("word"));
    assert_eq!(app.entries().len(), 2);
    assert!(app.get("missing").is_none());
}

#[test]
fn broken_text_is_an_error() {
    assert_eq!(parse_text(b"\"a\" { \"b\" \"c\""), Err(KvError::Truncated));
    assert_eq!(parse_text(b"\"a\" \"unterminated"), Err(KvError::Truncated));
    assert!(matches!(parse_text(b"}"), Err(KvError::Unexpected(0))));
}

/// Binary KeyValues as a package arrives (after its 4-byte prefix).
fn package() -> Vec<u8> {
    let mut b = Vec::new();
    let mut entry = |kind: u8, key: &str, value: &[u8]| {
        b.push(kind);
        b.extend_from_slice(key.as_bytes());
        b.push(0);
        b.extend_from_slice(value);
    };
    entry(0x00, "37", &[]);
    entry(0x02, "packageid", &37i32.to_le_bytes());
    entry(0x01, "note", b"text\0");
    entry(0x07, "big", &(1u64 << 40).to_le_bytes());
    entry(0x03, "ratio", &1.5f32.to_le_bytes());
    entry(0x00, "appids", &[]);
    entry(0x02, "0", &211i32.to_le_bytes());
    entry(0x02, "1", &215i32.to_le_bytes());
    b.push(0x08); // appids
    b.push(0x08); // 37
    b.push(0x08); // the top
    b
}

#[test]
fn binary_types_and_sections() {
    let kv = parse_binary(&package()).unwrap();
    let p = kv.get("37").unwrap();
    assert_eq!(p.get("packageid"), Some(&Kv::Int(37)));
    assert_eq!(p.get("note").and_then(Kv::text).as_deref(), Some("text"));
    assert_eq!(p.get("big").and_then(Kv::int), Some(1 << 40));
    assert_eq!(p.get("ratio"), Some(&Kv::Float(1.5)));
    let apps: Vec<i64> = p.get("appids").unwrap().entries().iter().filter_map(|(_, v)| v.int()).collect();
    assert_eq!(apps, [211, 215]);
}

#[test]
fn broken_binary_is_an_error() {
    let p = package();
    assert_eq!(parse_binary(&p[..p.len() - 3]), Err(KvError::Truncated));
    assert_eq!(parse_binary(&[0x09, b'k', 0]), Err(KvError::UnknownType(9)));
    assert_eq!(parse_binary(&[]).unwrap(), Kv::Map(vec![]));
}
