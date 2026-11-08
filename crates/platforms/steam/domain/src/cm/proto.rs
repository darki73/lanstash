//! Protocol Buffers' wire format, the part Steam's messages use: varints,
//! fixed 32 and 64 bits, and length-delimited bytes. What each field means is
//! written next to the message that has it.

use alloc::vec::Vec;

/// One field's value as it is on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

impl<'a> Wire<'a> {
    /// An integer field (varint or fixed).
    #[must_use]
    pub fn int(self) -> Option<u64> {
        match self {
            Self::Varint(v) | Self::Fixed64(v) => Some(v),
            Self::Fixed32(v) => Some(u64::from(v)),
            Self::Bytes(_) => None,
        }
    }

    /// A length-delimited field (bytes, string, or an embedded message).
    #[must_use]
    pub fn bytes(self) -> Option<&'a [u8]> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// A UTF-8 string field.
    #[must_use]
    pub fn str(self) -> Option<&'a str> {
        core::str::from_utf8(self.bytes()?).ok()
    }
}

/// A varint at `data[at..]`: its value and the index after it.
#[must_use]
pub fn read_varint(data: &[u8], mut at: usize) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *data.get(at)?;
        at += 1;
        v |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Some((v, at));
        }
    }
    None
}

/// Appends `v` as a varint.
pub fn put_varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// The fields of a message, in wire order; an `Err(())` item ends a malformed one.
#[derive(Debug, Clone)]
pub struct Fields<'a> {
    data: &'a [u8],
    at: usize,
}

/// Iterates over the fields of `data`.
#[must_use]
pub fn fields(data: &[u8]) -> Fields<'_> {
    Fields { data, at: 0 }
}

impl<'a> Fields<'a> {
    fn field(&mut self) -> Option<(u32, Wire<'a>)> {
        let (key, at) = read_varint(self.data, self.at)?;
        let number = u32::try_from(key >> 3).ok().filter(|n| *n > 0)?;
        let (wire, at) = match key & 7 {
            0 => {
                let (v, at) = read_varint(self.data, at)?;
                (Wire::Varint(v), at)
            }
            1 => (Wire::Fixed64(u64::from_le_bytes(self.data.get(at..at + 8)?.try_into().ok()?)), at + 8),
            2 => {
                let (n, at) = read_varint(self.data, at)?;
                let end = at.checked_add(usize::try_from(n).ok()?)?;
                (Wire::Bytes(self.data.get(at..end)?), end)
            }
            5 => (Wire::Fixed32(u32::from_le_bytes(self.data.get(at..at + 4)?.try_into().ok()?)), at + 4),
            _ => return None,
        };
        self.at = at;
        Some((number, wire))
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u32, Wire<'a>), ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.data.len() {
            return None;
        }
        Some(self.field().ok_or_else(|| {
            self.at = self.data.len();
        }))
    }
}

/// Builds a message field by field.
#[derive(Debug, Clone, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn key(&mut self, field: u32, wire: u64) {
        put_varint((u64::from(field) << 3) | wire, &mut self.buf);
    }

    /// An int32/int64/uint32/uint64/enum field. A negative int32 is passed sign-extended.
    #[must_use]
    pub fn varint(mut self, field: u32, v: u64) -> Self {
        self.key(field, 0);
        put_varint(v, &mut self.buf);
        self
    }

    #[must_use]
    pub fn bool(self, field: u32, v: bool) -> Self {
        self.varint(field, u64::from(v))
    }

    #[must_use]
    pub fn fixed64(mut self, field: u32, v: u64) -> Self {
        self.key(field, 1);
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    #[must_use]
    pub fn fixed32(mut self, field: u32, v: u32) -> Self {
        self.key(field, 5);
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// A bytes, string or embedded-message field.
    #[must_use]
    pub fn bytes(mut self, field: u32, v: &[u8]) -> Self {
        self.key(field, 2);
        put_varint(v.len() as u64, &mut self.buf);
        self.buf.extend_from_slice(v);
        self
    }

    #[must_use]
    pub fn string(self, field: u32, v: &str) -> Self {
        self.bytes(field, v.as_bytes())
    }

    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}
