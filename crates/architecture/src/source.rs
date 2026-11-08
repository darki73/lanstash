//! Rust source helpers: comments removed, and the `std` modules a file names.

use std::collections::BTreeSet;

/// Removes `//` and `/* */` comments (crudely: string literals containing
/// comment markers are rare in this codebase and only cause false positives).
#[must_use]
pub fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            let start = i;
            i += 1;
            while i < b.len() && b[i] != b'/' {
                i += 1;
            }
            out.push_str(&src[start..i]);
        }
    }
    out
}

/// Number of `#` of a raw string opener `r#*"` at `i`, if one starts there.
fn raw_string_hashes(b: &[u8], i: usize) -> Option<usize> {
    if b[i] != b'r' || (i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
        return None;
    }
    let hashes = b[i + 1..].iter().take_while(|c| **c == b'#').count();
    (b.get(i + 1 + hashes) == Some(&b'"')).then_some(hashes)
}

/// The code of a Rust source: comments removed and the contents of string and
/// character literals emptied (the quotes stay), so text inside them never
/// counts as code.
#[must_use]
pub fn code_only(src: &str) -> String {
    let code = strip_comments(src);
    let b = code.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        if let Some(hashes) = raw_string_hashes(b, i) {
            let open = i + 2 + hashes;
            let close: String = core::iter::once('"').chain(core::iter::repeat_n('#', hashes)).collect();
            let end = code[open..].find(&close).map_or(b.len(), |j| open + j);
            out.push_str(&code[copied..open]);
            i = (end + close.len()).min(b.len());
            out.push_str(&code[end.min(b.len())..i]);
            copied = i;
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            out.push_str(&code[copied..=i]);
            i = (j + 1).min(b.len());
            out.push_str(&code[j.min(b.len())..i]);
            copied = i;
        } else if b[i] == b'\'' && (b.get(i + 1) == Some(&b'\\') || b.get(i + 2) == Some(&b'\'')) {
            let mut j = i + 1;
            if b[j] == b'\\' {
                j += 2;
            }
            while j < b.len() && b[j] != b'\'' {
                j += 1;
            }
            out.push_str(&code[copied..=i]);
            i = (j + 1).min(b.len());
            out.push_str(&code[j.min(b.len())..i]);
            copied = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&code[copied.min(b.len())..]);
    out
}

/// The names of a `{a, b::c, d::{e}}` list, at its top level.
fn list_names(list: &str, out: &mut BTreeSet<String>) {
    let mut depth = 0;
    let mut start = 0;
    for (j, c) in list.char_indices().chain(std::iter::once((list.len(), ','))) {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                let name: String = list[start..j].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if !name.is_empty() {
                    out.insert(name);
                }
                start = j + 1;
            }
            _ => {}
        }
    }
}

/// Top-level `std` modules referenced by a source file, from paths like
/// `std::fs::File`, `use std::{fs, io::Read}`, `::std::net`.
#[must_use]
pub fn std_modules_used(src: &str) -> BTreeSet<String> {
    let code = code_only(src);
    let mut out = BTreeSet::new();
    let mut rest = code.as_str();
    while let Some(i) = rest.find("std::") {
        let before = rest[..i].chars().last();
        rest = &rest[i + 5..];
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let after = rest.trim_start();
        if let Some(inner) = after.strip_prefix('{') {
            let inner: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
            let mut depth = 1;
            let mut end = inner.len();
            for (j, c) in inner.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = j;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            list_names(&inner[..end], &mut out);
        } else {
            let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                out.insert(name);
            }
        }
    }
    out
}
