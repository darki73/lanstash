//! Reed-Solomon error correction over GF(2^8) with the QR code's field
//! polynomial `x^8 + x^4 + x^3 + x^2 + 1` (0x11D), generator roots 2^0..2^(n-1)
//! (ISO/IEC 18004 section 7.5.2).

use alloc::vec;
use alloc::vec::Vec;

/// `x * y` in GF(2^8) modulo 0x11D.
#[must_use]
pub fn mul(x: u8, y: u8) -> u8 {
    let mut z: u32 = 0;
    for i in (0..8).rev() {
        z = (z << 1) ^ ((z >> 7) * 0x11D);
        z ^= u32::from((y >> i) & 1) * u32::from(x);
    }
    z as u8
}

/// The generator polynomial of `degree`, highest coefficient (always 1) left out.
#[must_use]
pub fn divisor(degree: usize) -> Vec<u8> {
    let mut r = vec![0u8; degree];
    if let Some(last) = r.last_mut() {
        *last = 1;
    }
    let mut root = 1u8;
    for _ in 0..degree {
        for j in 0..degree {
            r[j] = mul(r[j], root);
            if j + 1 < degree {
                r[j] ^= r[j + 1];
            }
        }
        root = mul(root, 2);
    }
    r
}

/// The error-correction codewords of `data` for `divisor`.
#[must_use]
pub fn remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
    let mut r = vec![0u8; divisor.len()];
    for &b in data {
        let factor = b ^ r.first().copied().unwrap_or(0);
        r.rotate_left(1);
        if let Some(last) = r.last_mut() {
            *last = 0;
        }
        for (x, &y) in r.iter_mut().zip(divisor) {
            *x ^= mul(y, factor);
        }
    }
    r
}
