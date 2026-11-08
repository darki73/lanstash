//! QR codes (ISO/IEC 18004): byte mode, error correction level M, versions
//! 1 to 10 (up to 213 bytes), enough for a sign-in link. Written here
//! rather than taken from a crate; the module layout follows the standard's
//! steps. Rendering is in
//! [`render`], the error correction in [`rs`].

pub mod render;
pub mod rs;

use alloc::vec;
use alloc::vec::Vec;

/// Level M per version 1..=10: (error-correction codewords per block, blocks).
const BLOCKS_M: [(usize, usize); 10] =
    [(10, 1), (16, 1), (26, 1), (18, 2), (24, 2), (16, 4), (18, 4), (22, 4), (22, 5), (26, 5)];

/// Level M's two format bits.
const LEVEL_M: u32 = 0b00;

/// The largest payload (version 10, level M, byte mode).
pub const MAX_BYTES: usize = 213;

/// A QR code: `size` x `size` modules, `true` = dark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qr {
    pub version: u8,
    pub mask: u8,
    pub size: usize,
    dark: Vec<bool>,
}

impl Qr {
    /// The module at `row`, `col` (outside the code: light).
    #[must_use]
    pub fn dark(&self, row: usize, col: usize) -> bool {
        row < self.size && col < self.size && self.dark[row * self.size + col]
    }
}

/// All codewords (data and correction) of `version`.
fn raw_codewords(version: usize) -> usize {
    let v = version;
    let mut bits = (16 * v + 128) * v + 64;
    if v >= 2 {
        let n = v / 7 + 2;
        bits -= (25 * n - 10) * n - 55;
        if v >= 7 {
            bits -= 36;
        }
    }
    bits / 8
}

fn data_codewords(version: usize) -> usize {
    let (ec, blocks) = BLOCKS_M[version - 1];
    raw_codewords(version) - ec * blocks
}

/// Centres of the alignment patterns along one axis.
fn alignment(version: usize) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }
    let n = version / 7 + 2;
    let size = version * 4 + 17;
    let step = (version * 8 + n * 3 + 5) / (n * 4 - 4) * 2;
    let mut out: Vec<usize> = (0..n - 1).map(|i| size - 7 - i * step).collect();
    out.push(6);
    out.reverse();
    out
}

struct Grid {
    size: usize,
    dark: Vec<bool>,
    function: Vec<bool>,
}

impl Grid {
    fn set(&mut self, x: usize, y: usize, dark: bool) {
        let i = y * self.size + x;
        self.dark[i] = dark;
        self.function[i] = true;
    }

    fn finder(&mut self, cx: usize, cy: usize) {
        for dy in -4i32..=4 {
            for dx in -4i32..=4 {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if (0..self.size as i32).contains(&x) && (0..self.size as i32).contains(&y) {
                    let d = dx.abs().max(dy.abs());
                    self.set(x as usize, y as usize, d != 2 && d != 4);
                }
            }
        }
    }

    fn format(&mut self, mask: u8) {
        let data = (LEVEL_M << 3) | u32::from(mask);
        let mut rem = data;
        for _ in 0..10 {
            rem = (rem << 1) ^ ((rem >> 9) * 0x537);
        }
        let bits = ((data << 10) | rem) ^ 0x5412;
        let bit = |i: u32| (bits >> i) & 1 == 1;
        let s = self.size;
        for i in 0..=5 {
            self.set(8, i, bit(i as u32));
        }
        self.set(8, 7, bit(6));
        self.set(8, 8, bit(7));
        self.set(7, 8, bit(8));
        for i in 9..15 {
            self.set(14 - i, 8, bit(i as u32));
        }
        for i in 0..8 {
            self.set(s - 1 - i, 8, bit(i as u32));
        }
        for i in 8..15 {
            self.set(8, s - 15 + i, bit(i as u32));
        }
        self.set(8, s - 8, true);
    }

    fn version_bits(&mut self, version: usize) {
        if version < 7 {
            return;
        }
        let mut rem = version as u32;
        for _ in 0..12 {
            rem = (rem << 1) ^ ((rem >> 11) * 0x1F25);
        }
        let bits = ((version as u32) << 12) | rem;
        for i in 0..18 {
            let dark = (bits >> i) & 1 == 1;
            let (a, b) = (self.size - 11 + i % 3, i / 3);
            self.set(a, b, dark);
            self.set(b, a, dark);
        }
    }

    fn functions(version: usize) -> Self {
        let size = version * 4 + 17;
        let mut g = Self { size, dark: vec![false; size * size], function: vec![false; size * size] };
        for i in 0..size {
            g.set(6, i, i % 2 == 0);
            g.set(i, 6, i % 2 == 0);
        }
        g.finder(3, 3);
        g.finder(size - 4, 3);
        g.finder(3, size - 4);
        let pos = alignment(version);
        let n = pos.len();
        for (i, &x) in pos.iter().enumerate() {
            for (j, &y) in pos.iter().enumerate() {
                let corner = |k: usize| k == 0 || k == n - 1;
                if corner(i) && corner(j) && !(i == n - 1 && j == n - 1) {
                    continue;
                }
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let d = dx.abs().max(dy.abs());
                        g.set((x as i32 + dx) as usize, (y as i32 + dy) as usize, d != 1);
                    }
                }
            }
        }
        g.format(0);
        g.version_bits(version);
        g
    }

    /// Places the codewords in the zig-zag order, upwards and downwards in column pairs.
    fn place(&mut self, codewords: &[u8]) {
        let s = self.size as i32;
        let mut i = 0usize;
        let mut right = s - 1;
        while right >= 1 {
            if right == 6 {
                right = 5;
            }
            for vert in 0..s {
                for j in 0..2 {
                    let x = (right - j) as usize;
                    let upward = (right + 1) & 2 == 0;
                    let y = if upward { s - 1 - vert } else { vert } as usize;
                    let at = y * self.size + x;
                    if !self.function[at] && i < codewords.len() * 8 {
                        self.dark[at] = (codewords[i >> 3] >> (7 - (i & 7))) & 1 == 1;
                        i += 1;
                    }
                }
            }
            right -= 2;
        }
    }

    fn apply_mask(&mut self, mask: u8) {
        for y in 0..self.size {
            for x in 0..self.size {
                let flip = match mask {
                    0 => (x + y) % 2 == 0,
                    1 => y % 2 == 0,
                    2 => x % 3 == 0,
                    3 => (x + y) % 3 == 0,
                    4 => (x / 3 + y / 2) % 2 == 0,
                    5 => x * y % 2 + x * y % 3 == 0,
                    6 => (x * y % 2 + x * y % 3) % 2 == 0,
                    _ => ((x + y) % 2 + x * y % 3) % 2 == 0,
                };
                let at = y * self.size + x;
                if flip && !self.function[at] {
                    self.dark[at] = !self.dark[at];
                }
            }
        }
    }
}

/// The data codewords: mode, length, bytes, terminator, padding.
fn data_bits(data: &[u8], version: usize) -> Vec<u8> {
    let capacity = data_codewords(version) * 8;
    let mut bits: Vec<bool> = Vec::with_capacity(capacity);
    let mut push = |v: u32, n: u32| (0..n).rev().for_each(|i| bits.push((v >> i) & 1 == 1));
    push(0b0100, 4);
    push(data.len() as u32, if version < 10 { 8 } else { 16 });
    for &b in data {
        push(u32::from(b), 8);
    }
    let room = capacity - bits.len();
    bits.extend(core::iter::repeat_n(false, room.min(4)));
    while !bits.len().is_multiple_of(8) {
        bits.push(false);
    }
    let mut out: Vec<u8> = bits.chunks(8).map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | u8::from(b))).collect();
    for pad in [0xEC, 0x11].into_iter().cycle() {
        if out.len() * 8 >= capacity {
            break;
        }
        out.push(pad);
    }
    out
}

/// Splits into blocks, adds each block's correction, interleaves.
fn with_correction(data: &[u8], version: usize) -> Vec<u8> {
    let (ec, blocks) = BLOCKS_M[version - 1];
    let raw = raw_codewords(version);
    let short = blocks - raw % blocks;
    let short_len = raw / blocks;
    let div = rs::divisor(ec);
    let mut parts: Vec<Vec<u8>> = Vec::with_capacity(blocks);
    let mut at = 0;
    for i in 0..blocks {
        let len = short_len - ec + usize::from(i >= short);
        let block = &data[at..at + len];
        at += len;
        let mut b = block.to_vec();
        if i < short {
            b.push(0); // a hole, skipped when interleaving
        }
        b.extend(rs::remainder(block, &div));
        parts.push(b);
    }
    let mut out = Vec::with_capacity(raw);
    for i in 0..parts[0].len() {
        for (j, p) in parts.iter().enumerate() {
            if i != short_len - ec || j >= short {
                out.push(p[i]);
            }
        }
    }
    out
}

/// A penalty score (the standard's four rules); lower is easier to scan.
fn penalty(g: &Grid) -> u32 {
    let s = g.size;
    let at = |x: usize, y: usize| g.dark[y * s + x];
    let mut score = 0u32;
    for horizontal in [true, false] {
        for a in 0..s {
            let cell = |b: usize| if horizontal { at(b, a) } else { at(a, b) };
            let mut run = 1u32;
            for b in 1..s {
                if cell(b) == cell(b - 1) {
                    run += 1;
                    if run == 5 {
                        score += 3;
                    } else if run > 5 {
                        score += 1;
                    }
                } else {
                    run = 1;
                }
            }
            for b in 0..s.saturating_sub(10) {
                let w: Vec<bool> = (b..b + 11).map(cell).collect();
                let finder = [true, false, true, true, true, false, true];
                let light4 = |r: &[bool]| r.iter().all(|d| !d);
                if (w[..7] == finder && light4(&w[7..])) || (light4(&w[..4]) && w[4..] == finder) {
                    score += 40;
                }
            }
        }
    }
    for y in 0..s - 1 {
        for x in 0..s - 1 {
            let c = at(x, y);
            if c == at(x + 1, y) && c == at(x, y + 1) && c == at(x + 1, y + 1) {
                score += 3;
            }
        }
    }
    let dark = g.dark.iter().filter(|d| **d).count() as u32;
    let total = (s * s) as u32;
    let k = (dark * 20).abs_diff(total * 10).div_ceil(total).saturating_sub(1);
    score + k * 10
}

/// Encodes `data`, with `mask` 0..=7, or the mask that scores best when `None`.
/// `None` when `data` is longer than [`MAX_BYTES`].
#[must_use]
pub fn encode(data: &[u8], mask: Option<u8>) -> Option<Qr> {
    let version = (1..=10).find(|&v| {
        let header = 4 + if v < 10 { 8 } else { 16 };
        header + data.len() * 8 <= data_codewords(v) * 8 && (v >= 10 || data.len() < 256)
    })?;
    let codewords = with_correction(&data_bits(data, version), version);
    let mut base = Grid::functions(version);
    base.place(&codewords);
    let masked = |m: u8| {
        let mut g = Grid { size: base.size, dark: base.dark.clone(), function: base.function.clone() };
        g.apply_mask(m);
        g.format(m);
        g
    };
    let (mask, grid) = match mask {
        Some(m) => (m & 7, masked(m & 7)),
        None => (0..8).map(|m| (m, masked(m))).min_by_key(|(_, g)| penalty(g))?,
    };
    Some(Qr { version: version as u8, mask, size: grid.size, dark: grid.dark })
}
