//! RSA public-key encryption with PKCS#1 v1.5 padding (RFC 8017 section
//! 7.2.1): Steam's logon sends the password encrypted with the key its
//! authentication service hands out. Encryption only; plain big-integer
//! arithmetic, not constant time, which is fine here because only public
//! values go through it (the padding and the password are encrypted, not
//! computed with a secret key).

use alloc::vec;
use alloc::vec::Vec;

/// Little-endian 32-bit limbs of `be`, at least `limbs` long.
fn limbs_of(be: &[u8], limbs: usize) -> Vec<u32> {
    let mut out = vec![0u32; limbs.max(be.len().div_ceil(4))];
    for (i, b) in be.iter().rev().enumerate() {
        out[i / 4] |= u32::from(*b) << (8 * (i % 4));
    }
    out
}

/// `x` as `len` big-endian bytes (high bytes beyond `len` must be zero).
fn bytes_of(x: &[u32], len: usize) -> Vec<u8> {
    (0..len).rev().map(|i| x.get(i / 4).map_or(0, |l| (l >> (8 * (i % 4))) as u8)).collect()
}

fn ge(a: &[u32], b: &[u32]) -> bool {
    for i in (0..a.len().max(b.len())).rev() {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    true
}

/// `a -= b`, given `a >= b`.
fn sub(a: &mut [u32], b: &[u32]) {
    let mut borrow = 0u64;
    for (i, x) in a.iter_mut().enumerate() {
        let y = u64::from(b.get(i).copied().unwrap_or(0)) + borrow;
        let (d, under) = (u64::from(*x).wrapping_sub(y), u64::from(*x) < y);
        *x = d as u32;
        borrow = u64::from(under);
    }
}

fn mul(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0u64;
        for (j, &y) in b.iter().enumerate() {
            let t = u64::from(x) * u64::from(y) + u64::from(out[i + j]) + carry;
            out[i + j] = t as u32;
            carry = t >> 32;
        }
        out[i + b.len()] = carry as u32;
    }
    out
}

/// `x mod m`, by shift and subtract, one bit at a time.
fn reduce(x: &[u32], m: &[u32]) -> Vec<u32> {
    let mut r = vec![0u32; m.len() + 1];
    for i in (0..x.len() * 32).rev() {
        let mut carry = (x[i / 32] >> (i % 32)) & 1;
        for l in &mut r {
            let next = *l >> 31;
            *l = (*l << 1) | carry;
            carry = next;
        }
        if ge(&r, m) {
            sub(&mut r, m);
        }
    }
    r.truncate(m.len());
    r
}

/// `base ^ exp mod modulus`, all big-endian; the result is as long as the modulus.
/// `None` for a zero modulus.
#[must_use]
pub fn modpow(base: &[u8], exp: &[u8], modulus: &[u8]) -> Option<Vec<u8>> {
    let m = limbs_of(modulus, 0);
    if m.iter().all(|l| *l == 0) {
        return None;
    }
    let b = reduce(&limbs_of(base, 0), &m);
    let mut r = reduce(&[1], &m);
    for byte in exp {
        for bit in (0..8).rev() {
            r = reduce(&mul(&r, &r), &m);
            if (byte >> bit) & 1 == 1 {
                r = reduce(&mul(&r, &b), &m);
            }
        }
    }
    Some(bytes_of(&r, modulus.len()))
}

/// An RSA public key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    modulus: Vec<u8>,
    exponent: Vec<u8>,
}

impl PublicKey {
    /// A key from its big-endian modulus and exponent; `None` for a modulus
    /// under 512 bits or an even one.
    #[must_use]
    pub fn new(modulus: &[u8], exponent: &[u8]) -> Option<Self> {
        let modulus: Vec<u8> = modulus.iter().copied().skip_while(|b| *b == 0).collect();
        if modulus.len() < 64 || modulus.last().is_none_or(|b| b & 1 == 0) {
            return None;
        }
        Some(Self { modulus, exponent: exponent.to_vec() })
    }

    /// The modulus length in bytes (`k` in RFC 8017).
    #[must_use]
    pub fn size(&self) -> usize {
        self.modulus.len()
    }

    /// RSAES-PKCS1-v1_5: `00 02 PS 00 message`, PS being at least 8 nonzero
    /// bytes taken in order from `random` (zeros skipped), then raised to
    /// the exponent. `None` when the message is longer than `size() - 11`
    /// or `random` has too few nonzero bytes.
    #[must_use]
    pub fn encrypt_pkcs1(&self, message: &[u8], random: &[u8]) -> Option<Vec<u8>> {
        let k = self.size();
        let pad = k.checked_sub(message.len() + 3).filter(|p| *p >= 8)?;
        let ps: Vec<u8> = random.iter().copied().filter(|b| *b != 0).take(pad).collect();
        if ps.len() < pad {
            return None;
        }
        let mut em = Vec::with_capacity(k);
        em.extend_from_slice(&[0, 2]);
        em.extend_from_slice(&ps);
        em.push(0);
        em.extend_from_slice(message);
        modpow(&em, &self.exponent, &self.modulus)
    }
}
