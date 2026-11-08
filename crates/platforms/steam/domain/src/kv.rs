//! Valve's KeyValues, the two forms PICS answers with: text (`"key" "value"`
//! and `"key" { ... }`, an app's info) and binary (typed, a package's info).
//! Keys keep their order; lookups ignore case, as Valve's do.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A value: text, a number (binary form only), or a section of named values.
#[derive(Debug, Clone, PartialEq)]
pub enum Kv {
    Str(String),
    Int(i64),
    Float(f32),
    Map(Vec<(String, Kv)>),
}

impl Kv {
    /// The value under `key` in a section.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Kv> {
        match self {
            Self::Map(m) => m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value at a path of keys.
    #[must_use]
    pub fn at(&self, path: &[&str]) -> Option<&Kv> {
        path.iter().try_fold(self, |v, k| v.get(k))
    }

    /// A section's entries (none for a single value).
    #[must_use]
    pub fn entries(&self) -> &[(String, Kv)] {
        match self {
            Self::Map(m) => m,
            _ => &[],
        }
    }

    /// Text, or a number written as text.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        match self {
            Self::Str(s) => Some(s.clone()),
            Self::Int(i) => Some(i.to_string()),
            Self::Float(f) => Some(alloc::format!("{f}")),
            Self::Map(_) => None,
        }
    }

    /// A whole number, from either form.
    #[must_use]
    pub fn int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
}

/// Why KeyValues could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KvError {
    /// It ended inside a key, a value or a section.
    Truncated,
    /// Something else where a key or value was expected (at this byte).
    Unexpected(usize),
    /// A binary value of an unknown type (this type byte).
    UnknownType(u8),
}

/// Deepest nesting accepted.
const MAX_DEPTH: usize = 64;

struct Text<'a> {
    s: &'a [u8],
    at: usize,
}

impl Text<'_> {
    /// Skips spaces, line ends and `//` comments.
    fn skip(&mut self) {
        loop {
            while self.s.get(self.at).is_some_and(u8::is_ascii_whitespace) {
                self.at += 1;
            }
            if self.s.get(self.at..self.at + 2) == Some(b"//") {
                while self.s.get(self.at).is_some_and(|b| *b != b'\n') {
                    self.at += 1;
                }
            } else {
                return;
            }
        }
    }

    /// A quoted or bare token; `None` at a brace or the end.
    fn token(&mut self) -> Result<Option<String>, KvError> {
        self.skip();
        match self.s.get(self.at) {
            None | Some(b'{' | b'}' | 0) => Ok(None),
            Some(b'"') => {
                self.at += 1;
                let mut out = Vec::new();
                loop {
                    match *self.s.get(self.at).ok_or(KvError::Truncated)? {
                        b'"' => break,
                        b'\\' => {
                            self.at += 1;
                            out.push(match *self.s.get(self.at).ok_or(KvError::Truncated)? {
                                b'n' => b'\n',
                                b't' => b'\t',
                                other => other,
                            });
                        }
                        b => out.push(b),
                    }
                    self.at += 1;
                }
                self.at += 1;
                Ok(Some(String::from_utf8_lossy(&out).into_owned()))
            }
            Some(_) => {
                let start = self.at;
                while self.s.get(self.at).is_some_and(|b| !b.is_ascii_whitespace() && !b"{}\"".contains(b)) {
                    self.at += 1;
                }
                Ok(Some(String::from_utf8_lossy(&self.s[start..self.at]).into_owned()))
            }
        }
    }

    /// Entries up to a closing brace (or the end at the top).
    fn section(&mut self, depth: usize, top: bool) -> Result<Vec<(String, Kv)>, KvError> {
        if depth > MAX_DEPTH {
            return Err(KvError::Unexpected(self.at));
        }
        let mut out = Vec::new();
        loop {
            let Some(key) = self.token()? else {
                match self.s.get(self.at) {
                    Some(b'}') if !top => {
                        self.at += 1;
                        return Ok(out);
                    }
                    None | Some(0) if top => return Ok(out),
                    None | Some(0) => return Err(KvError::Truncated),
                    _ => return Err(KvError::Unexpected(self.at)),
                }
            };
            self.skip();
            if self.s.get(self.at) == Some(&b'{') {
                self.at += 1;
                out.push((key, Kv::Map(self.section(depth + 1, false)?)));
            } else {
                let value = self.token()?.ok_or(KvError::Unexpected(self.at))?;
                out.push((key, Kv::Str(value)));
            }
        }
    }
}

/// Reads text KeyValues: the top level's entries as one section (a trailing
/// NUL, as PICS sends, ends it).
///
/// # Errors
/// [`KvError`].
pub fn parse_text(data: &[u8]) -> Result<Kv, KvError> {
    Text { s: data, at: 0 }.section(0, true).map(Kv::Map)
}

struct Binary<'a> {
    s: &'a [u8],
    at: usize,
}

impl Binary<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], KvError> {
        let b = self.s.get(self.at..self.at + n).ok_or(KvError::Truncated)?;
        self.at += n;
        Ok(b)
    }

    fn cstr(&mut self) -> Result<String, KvError> {
        let rest = self.s.get(self.at..).ok_or(KvError::Truncated)?;
        let n = rest.iter().position(|b| *b == 0).ok_or(KvError::Truncated)?;
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.at += n + 1;
        Ok(s)
    }

    /// One entry of type `kind` (its name and value), the type byte already read.
    fn entry(&mut self, kind: u8, depth: usize) -> Result<(String, Kv), KvError> {
        let key = self.cstr()?;
        let value = match kind {
            0x00 => Kv::Map(self.section(depth + 1)?),
            0x01 => Kv::Str(self.cstr()?),
            0x02 | 0x04 | 0x06 => {
                let b = self.take(4)?;
                Kv::Int(i64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])))
            }
            0x03 => {
                let b = self.take(4)?;
                Kv::Float(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            0x07 | 0x0A => {
                let mut a = [0u8; 8];
                a.copy_from_slice(self.take(8)?);
                Kv::Int(i64::from_le_bytes(a))
            }
            other => return Err(KvError::UnknownType(other)),
        };
        Ok((key, value))
    }

    /// A section's entries, up to its 0x08.
    fn section(&mut self, depth: usize) -> Result<Vec<(String, Kv)>, KvError> {
        if depth > MAX_DEPTH {
            return Err(KvError::Unexpected(self.at));
        }
        let mut out = Vec::new();
        loop {
            let kind = *self.take(1)?.first().ok_or(KvError::Truncated)?;
            if kind == 0x08 {
                return Ok(out);
            }
            out.push(self.entry(kind, depth)?);
        }
    }
}

/// Reads binary KeyValues (type byte, NUL-ended name, value; 0x08 ends a
/// section): the top level's entries as one section.
///
/// # Errors
/// [`KvError`].
pub fn parse_binary(data: &[u8]) -> Result<Kv, KvError> {
    let mut b = Binary { s: data, at: 0 };
    let mut out = Vec::new();
    while let Some(&kind) = data.get(b.at).filter(|k| **k != 0x08) {
        b.at += 1;
        out.push(b.entry(kind, 0)?);
    }
    Ok(Kv::Map(out))
}
