//! A small JSON reader (RFC 8259) for the documents lanstash reads from
//! CDNs, such as Battle.net's catalog. Values are owned; objects keep their
//! members in order. Nesting is limited to [`MAX_DEPTH`] so a hostile
//! document cannot exhaust the stack.
//!
//! Writing is [`string`], and [`write`] for a document read and changed (a
//! whole number is written without a fraction, so sizes and times stay as
//! they were); otherwise the code that owns a document writes it.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

/// `s` as a JSON string, quoted and escaped.
#[must_use]
pub fn string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", u32::from(c));
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// `v` as JSON text, without spaces.
#[must_use]
pub fn write(v: &Value) -> String {
    let mut o = String::new();
    write_into(&mut o, v);
    o
}

/// The largest whole number written without a fraction: every integer up to it is exact in an f64.
const EXACT: f64 = 9_007_199_254_740_992.0;

fn write_into(o: &mut String, v: &Value) {
    match v {
        Value::Null => o.push_str("null"),
        Value::Bool(b) => o.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) if !n.is_finite() => o.push_str("null"),
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::float_cmp)]
        Value::Number(n) if (-EXACT..=EXACT).contains(n) && (*n as i64) as f64 == *n => {
            let _ = write!(o, "{}", *n as i64);
        }
        Value::Number(n) => {
            let _ = write!(o, "{n}");
        }
        Value::Str(s) => o.push_str(&string(s)),
        Value::Array(a) => {
            o.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                write_into(o, x);
            }
            o.push(']');
        }
        Value::Object(m) => {
            o.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                o.push_str(&string(k));
                o.push(':');
                write_into(o, x);
            }
            o.push('}');
        }
    }
}

/// Deepest nesting accepted.
pub const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member `key` of an object (the first, if repeated).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> &[Value] {
        match self {
            Value::Array(a) => a,
            _ => &[],
        }
    }

    /// An object's members; empty for anything else.
    #[must_use]
    pub fn members(&self) -> &[(String, Value)] {
        match self {
            Value::Object(m) => m,
            _ => &[],
        }
    }
}

/// Why a document is not JSON: the byte offset and what was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub at: usize,
    pub what: &'static str,
}

/// Parses a whole document.
///
/// # Errors
/// [`JsonError`] at the first byte that is not valid JSON.
pub fn parse(text: &str) -> Result<Value, JsonError> {
    let mut p = Parser { b: text.as_bytes(), i: 0 };
    p.ws();
    if p.b.get(p.i..p.i + 3) == Some(b"\xEF\xBB\xBF") {
        p.i += 3;
        p.ws();
    }
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() {
        return Err(p.err("trailing characters"));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err(&self, what: &'static str) -> JsonError {
        JsonError { at: self.i, what }
    }

    fn ws(&mut self) {
        while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self, w: &[u8], v: Value) -> Result<Value, JsonError> {
        if self.b.get(self.i..self.i + w.len()) == Some(w) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(self.err("unknown word"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nested too deeply"));
        }
        match self.b.get(self.i) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::Str),
            Some(b't') => self.word(b"true", Value::Bool(true)),
            Some(b'f') => self.word(b"false", Value::Bool(false)),
            Some(b'n') => self.word(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
            None => Err(self.err("unexpected end")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.i += 1;
        let mut m = Vec::new();
        self.ws();
        if self.eat(b'}') {
            return Ok(Value::Object(m));
        }
        loop {
            self.ws();
            if self.b.get(self.i) != Some(&b'"') {
                return Err(self.err("expected a key"));
            }
            let k = self.string()?;
            self.ws();
            if !self.eat(b':') {
                return Err(self.err("expected ':'"));
            }
            self.ws();
            let v = self.value(depth + 1)?;
            m.push((k, v));
            self.ws();
            if self.eat(b'}') {
                return Ok(Value::Object(m));
            }
            if !self.eat(b',') {
                return Err(self.err("expected ',' or '}'"));
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.i += 1;
        let mut a = Vec::new();
        self.ws();
        if self.eat(b']') {
            return Ok(Value::Array(a));
        }
        loop {
            self.ws();
            a.push(self.value(depth + 1)?);
            self.ws();
            if self.eat(b']') {
                return Ok(Value::Array(a));
            }
            if !self.eat(b',') {
                return Err(self.err("expected ',' or ']'"));
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let h = self.b.get(self.i..self.i + 4).ok_or_else(|| self.err("short \\u escape"))?;
        let s = core::str::from_utf8(h).map_err(|_| self.err("bad \\u escape"))?;
        let v = u32::from_str_radix(s, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while let Some(&c) = self.b.get(self.i) {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.i += 1;
            }
            // The input is a &str and runs stop at ASCII bytes, so this slice is UTF-8.
            out.push_str(core::str::from_utf8(&self.b[start..self.i]).map_err(|_| self.err("bad UTF-8"))?);
            match self.b.get(self.i) {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let e = *self.b.get(self.i).ok_or_else(|| self.err("unexpected end"))?;
                    self.i += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode()?,
                        _ => return Err(self.err("unknown escape")),
                    };
                    out.push(c);
                }
                Some(_) => return Err(self.err("control character in a string")),
                None => return Err(self.err("unterminated string")),
            }
        }
    }

    fn unicode(&mut self) -> Result<char, JsonError> {
        let hi = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&hi) {
            if self.b.get(self.i..self.i + 2) != Some(b"\\u") {
                return Err(self.err("lone surrogate"));
            }
            self.i += 2;
            let lo = self.hex4()?;
            if !(0xDC00..0xE000).contains(&lo) {
                return Err(self.err("lone surrogate"));
            }
            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
        } else {
            hi
        };
        char::from_u32(code).ok_or_else(|| self.err("lone surrogate"))
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        self.eat(b'-');
        let digits = |p: &mut Self| {
            let s = p.i;
            while matches!(p.b.get(p.i), Some(b'0'..=b'9')) {
                p.i += 1;
            }
            p.i > s
        };
        if self.b.get(self.i) == Some(&b'0') {
            self.i += 1;
        } else if !digits(self) {
            return Err(self.err("expected a digit"));
        }
        if self.eat(b'.') && !digits(self) {
            return Err(self.err("expected a digit"));
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if !self.eat(b'+') {
                self.eat(b'-');
            }
            if !digits(self) {
                return Err(self.err("expected a digit"));
            }
        }
        let s = core::str::from_utf8(&self.b[start..self.i]).map_err(|_| self.err("bad number"))?;
        s.parse::<f64>().map(Value::Number).map_err(|_| self.err("bad number"))
    }
}
