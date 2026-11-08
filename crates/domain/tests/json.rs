//! Tests for `lanstash_domain::json`.

use lanstash_domain::json::{MAX_DEPTH, Value, parse, string, write};

#[test]
fn a_document_read_is_written_back_the_same() {
    let text = r#"{"a":[1,2.5,-3,null,true,false],"size":3887784190176,"s":"q\"uote","e":{},"l":[]}"#;
    assert_eq!(write(&parse(text).unwrap()), text);
    assert_eq!(write(&Value::Number(f64::NAN)), "null");
    assert_eq!(
        write(&Value::Number(1e300)),
        "1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    );
}

#[test]
fn strings_are_written_quoted_and_escaped() {
    assert_eq!(string("a\"b\\c"), r#""a\"b\\c""#);
    assert_eq!(string("line\nnext\u{1}"), r#""line\nnext\u0001""#);
    assert_eq!(string("Diablo® II"), "\"Diablo® II\"");
}

#[test]
fn reads_every_kind_of_value() {
    let v = parse(r#" {"a": [1, -2.5e2, true, false, null], "b": {"c": "x\"\\\/\né😀"}, "a": 0} "#).unwrap();
    assert_eq!(
        v.get("a").unwrap().as_array(),
        &[Value::Number(1.0), Value::Number(-250.0), Value::Bool(true), Value::Bool(false), Value::Null]
    );
    assert_eq!(v.get("b").and_then(|b| b.get("c")).and_then(Value::as_str), Some("x\"\\/\n\u{e9}\u{1F600}"));
    assert_eq!(v.members().len(), 3, "members in order, repeats kept; get takes the first");
    assert_eq!(parse("[]").unwrap(), Value::Array(vec![]));
    assert_eq!(parse("{}").unwrap().get("x"), None);
    assert_eq!(parse("\u{feff}\"\u{fc}\"").unwrap().as_str(), Some("\u{fc}"), "a byte order mark is skipped");
}

#[test]
fn refuses_what_is_not_json() {
    let bad =
        ["", "{", "[1,]", r#"{"a" 1}"#, "01", "1.", "-", r#""\x""#, "\"a\nb\"", r#""\ud800""#, "tru", "1 2", "{'a':1}"];
    for b in bad {
        assert!(parse(b).is_err(), "{b:?}");
    }
    let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
    assert_eq!(parse(&deep).unwrap_err().what, "nested too deeply");
    let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
    assert!(parse(&ok).is_ok());
}
