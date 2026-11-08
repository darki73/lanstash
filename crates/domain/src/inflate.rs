//! DEFLATE (RFC 1951) and zlib (RFC 1950) decompression, for the compressed
//! chunks of Battle.net's BLTE files ([`crate::blte`]). Written here rather
//! than taken from a crate (lanstash depends on none). A straightforward
//! canonical-Huffman decoder: decoding is bit by bit, fast enough for
//! manifests of a few MB.
//!
//! Every output is bounded by a caller's limit, so a hostile stream cannot
//! make it allocate without end.

use alloc::vec::Vec;

/// Why a stream could not be decompressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateError {
    /// The input ended inside the stream.
    Truncated,
    /// Not a valid zlib header (method, window, check bits, or a preset dictionary).
    BadHeader,
    /// A reserved block type, a bad stored length, an invalid code or distance.
    Corrupt,
    /// The Adler-32 of the output does not match the stream's.
    BadChecksum,
    /// The output would exceed the caller's limit.
    TooLarge,
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    bits: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> Result<u32, InflateError> {
        while self.bits < n {
            let b = *self.data.get(self.pos).ok_or(InflateError::Truncated)?;
            self.pos += 1;
            self.bit |= u32::from(b) << self.bits;
            self.bits += 8;
        }
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.bits -= n;
        Ok(v)
    }

    /// Drops the bits left in the current byte.
    fn align(&mut self) {
        self.bit = 0;
        self.bits = 0;
    }
}

const MAX_BITS: usize = 15;

/// A canonical Huffman code: how many codes of each length, and the symbols in code order.
struct Huffman {
    count: [u16; MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl Huffman {
    /// Builds a code from code lengths; incomplete codes are allowed only
    /// where DEFLATE allows them (a single distance code).
    fn new(lengths: &[u8]) -> Result<Self, InflateError> {
        let mut count = [0u16; MAX_BITS + 1];
        for &l in lengths {
            count[usize::from(l)] += 1;
        }
        count[0] = 0;
        let mut left: i32 = 1;
        for &c in &count[1..] {
            left = (left << 1) - i32::from(c);
            if left < 0 {
                return Err(InflateError::Corrupt); // over-subscribed
            }
        }
        let mut offs = [0u16; MAX_BITS + 1];
        for len in 1..MAX_BITS {
            offs[len + 1] = offs[len] + count[len];
        }
        let mut symbol = alloc::vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[usize::from(offs[usize::from(l)])] = s as u16;
                offs[usize::from(l)] += 1;
            }
        }
        Ok(Self { count, symbol })
    }

    fn decode(&self, b: &mut Bits<'_>) -> Result<u16, InflateError> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..=MAX_BITS {
            code |= b.need(1)? as i32;
            let count = i32::from(self.count[len]);
            if code - count < first {
                return self.symbol.get((index + (code - first)) as usize).copied().ok_or(InflateError::Corrupt);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(InflateError::Corrupt)
    }
}

const LEN_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// The order code-length code lengths are sent in.
const CL_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn codes(b: &mut Bits<'_>, out: &mut Vec<u8>, max: usize, lit: &Huffman, dist: &Huffman) -> Result<(), InflateError> {
    loop {
        let sym = lit.decode(b)?;
        if sym < 256 {
            if out.len() >= max {
                return Err(InflateError::TooLarge);
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let i = usize::from(sym - 257);
            let len =
                usize::from(*LEN_BASE.get(i).ok_or(InflateError::Corrupt)?) + b.need(u32::from(LEN_EXTRA[i]))? as usize;
            let d = usize::from(dist.decode(b)?);
            let back = usize::from(*DIST_BASE.get(d).ok_or(InflateError::Corrupt)?)
                + b.need(u32::from(DIST_EXTRA[d]))? as usize;
            if back > out.len() {
                return Err(InflateError::Corrupt);
            }
            if out.len() + len > max {
                return Err(InflateError::TooLarge);
            }
            let start = out.len() - back;
            for k in 0..len {
                out.push(out[start + k]);
            }
        }
    }
}

fn fixed() -> Result<(Huffman, Huffman), InflateError> {
    let mut l = [0u8; 288];
    l[..144].fill(8);
    l[144..256].fill(9);
    l[256..280].fill(7);
    l[280..].fill(8);
    Ok((Huffman::new(&l)?, Huffman::new(&[5u8; 30])?))
}

fn dynamic(b: &mut Bits<'_>) -> Result<(Huffman, Huffman), InflateError> {
    let nlen = b.need(5)? as usize + 257;
    let ndist = b.need(5)? as usize + 1;
    let ncode = b.need(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err(InflateError::Corrupt);
    }
    let mut cl = [0u8; 19];
    for &i in &CL_ORDER[..ncode] {
        cl[i] = b.need(3)? as u8;
    }
    let clh = Huffman::new(&cl)?;
    let mut lengths = alloc::vec![0u8; nlen + ndist];
    let mut i = 0;
    while i < nlen + ndist {
        let sym = clh.decode(b)?;
        let (value, repeat) = match sym {
            0..=15 => (sym as u8, 1),
            16 => (*lengths.get(i.wrapping_sub(1)).ok_or(InflateError::Corrupt)?, 3 + b.need(2)? as usize),
            17 => (0, 3 + b.need(3)? as usize),
            18 => (0, 11 + b.need(7)? as usize),
            _ => return Err(InflateError::Corrupt),
        };
        if i + repeat > nlen + ndist {
            return Err(InflateError::Corrupt);
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    if lengths[256] == 0 {
        return Err(InflateError::Corrupt); // no end-of-block code
    }
    Ok((Huffman::new(&lengths[..nlen])?, Huffman::new(&lengths[nlen..])?))
}

/// Decompresses a raw DEFLATE stream; returns the output and the bytes of
/// input it used.
///
/// # Errors
/// [`InflateError`].
pub fn inflate(data: &[u8], max: usize) -> Result<(Vec<u8>, usize), InflateError> {
    let mut b = Bits { data, pos: 0, bit: 0, bits: 0 };
    let mut out = Vec::new();
    loop {
        let last = b.need(1)? == 1;
        match b.need(2)? {
            0 => {
                b.align();
                let h = b.data.get(b.pos..b.pos + 4).ok_or(InflateError::Truncated)?;
                let (len, nlen) = (u16::from_le_bytes([h[0], h[1]]), u16::from_le_bytes([h[2], h[3]]));
                if len != !nlen {
                    return Err(InflateError::Corrupt);
                }
                b.pos += 4;
                let block = b.data.get(b.pos..b.pos + usize::from(len)).ok_or(InflateError::Truncated)?;
                if out.len() + block.len() > max {
                    return Err(InflateError::TooLarge);
                }
                out.extend_from_slice(block);
                b.pos += usize::from(len);
            }
            1 => {
                let (lit, dist) = fixed()?;
                codes(&mut b, &mut out, max, &lit, &dist)?;
            }
            2 => {
                let (lit, dist) = dynamic(&mut b)?;
                codes(&mut b, &mut out, max, &lit, &dist)?;
            }
            _ => return Err(InflateError::Corrupt),
        }
        if last {
            return Ok((out, b.pos));
        }
    }
}

/// Adler-32 (RFC 1950).
#[must_use]
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

/// Decompresses a zlib stream (header, DEFLATE, Adler-32), at most `max` bytes.
///
/// # Errors
/// [`InflateError`].
pub fn zlib(data: &[u8], max: usize) -> Result<Vec<u8>, InflateError> {
    let (&cmf, &flg) = (data.first().ok_or(InflateError::Truncated)?, data.get(1).ok_or(InflateError::Truncated)?);
    if cmf & 0x0f != 8 || cmf >> 4 > 7 || (u16::from(cmf) << 8 | u16::from(flg)) % 31 != 0 || flg & 0x20 != 0 {
        return Err(InflateError::BadHeader);
    }
    let (out, used) = inflate(&data[2..], max)?;
    let sum = data.get(2 + used..2 + used + 4).ok_or(InflateError::Truncated)?;
    if u32::from_be_bytes([sum[0], sum[1], sum[2], sum[3]]) != adler32(&out) {
        return Err(InflateError::BadChecksum);
    }
    Ok(out)
}

/// Decompresses a gzip member (RFC 1952: header, DEFLATE, CRC-32, size), at
/// most `max` bytes. Steam's Multi messages carry their bodies this way.
///
/// # Errors
/// [`InflateError`]; `BadChecksum` when the CRC-32 or the size disagrees.
pub fn gzip(data: &[u8], max: usize) -> Result<Vec<u8>, InflateError> {
    let head = data.get(..10).ok_or(InflateError::Truncated)?;
    if head[..3] != [0x1f, 0x8b, 8] || head[3] & 0xE0 != 0 {
        return Err(InflateError::BadHeader);
    }
    let flags = head[3];
    let mut at = 10;
    if flags & 0x04 != 0 {
        let n = data.get(at..at + 2).ok_or(InflateError::Truncated)?;
        at += 2 + usize::from(u16::from_le_bytes([n[0], n[1]]));
    }
    for bit in [0x08, 0x10] {
        if flags & bit != 0 {
            let rest = data.get(at..).ok_or(InflateError::Truncated)?;
            at += 1 + rest.iter().position(|&b| b == 0).ok_or(InflateError::Truncated)?;
        }
    }
    if flags & 0x02 != 0 {
        at += 2;
    }
    let (out, used) = inflate(data.get(at..).ok_or(InflateError::Truncated)?, max)?;
    let t = data.get(at + used..at + used + 8).ok_or(InflateError::Truncated)?;
    let (crc, size) = (u32::from_le_bytes([t[0], t[1], t[2], t[3]]), u32::from_le_bytes([t[4], t[5], t[6], t[7]]));
    if crc != crate::checksum::crc::crc32(&out) || size != out.len() as u32 {
        return Err(InflateError::BadChecksum);
    }
    Ok(out)
}
