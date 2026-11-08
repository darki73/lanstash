//! Tests for `lanstash_domain::segment::error`.

use lanstash_domain::segment::error::FormatError;

#[test]
fn says_what_is_wrong() {
    let cases = [
        (FormatError::TooShort { what: "header", needed: 64, got: 3 }, "header: need 64 bytes, got 3"),
        (FormatError::BadMagic { what: "footer" }, "footer: bad magic"),
        (FormatError::UnsupportedVersion(9), "unsupported segment format version 9"),
        (
            FormatError::ChecksumMismatch { what: "record", stored: 1, computed: 0xabc },
            "record: checksum mismatch (stored 0x00000001, computed 0x00000abc)",
        ),
        (FormatError::InvalidField { what: "flags", value: 7 }, "flags: invalid value 7"),
        (FormatError::KeyNotUtf8, "record key is not valid UTF-8"),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text);
    }
}
