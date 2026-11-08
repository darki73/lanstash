//! Just enough TLS to read the server name (SNI) from a ClientHello, for
//! passing HTTPS through by host name. Nothing is decrypted.

use alloc::string::String;
use alloc::vec::Vec;

/// Largest ClientHello accepted (handshake bytes, possibly over several records).
pub const MAX_HELLO: usize = 64 * 1024;

/// Outcome of looking for the SNI host in the bytes received so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sni {
    /// The ClientHello is not complete yet.
    NeedMore,
    /// Not a TLS ClientHello (or malformed, or too large).
    NotTls,
    /// A valid ClientHello without a host name.
    NoName,
    /// The host name, lower-cased.
    Host(String),
}

/// Reassembles the handshake message from TLS records. `Err(())`: not TLS.
fn handshake_bytes(buf: &[u8]) -> Result<Option<Vec<u8>>, ()> {
    let mut hs: Vec<u8> = Vec::new();
    let mut pos = 0;
    loop {
        if let Some(n) = needed_handshake_len(&hs)
            && hs.len() >= n
        {
            hs.truncate(n);
            return Ok(Some(hs));
        }
        let Some(head) = buf.get(pos..pos + 5) else { return Ok(None) };
        if head[0] != 0x16 || head[1] != 0x03 {
            return Err(());
        }
        let len = usize::from(u16::from_be_bytes([head[3], head[4]]));
        if len == 0 || len > 18_432 {
            return Err(());
        }
        let Some(frag) = buf.get(pos + 5..pos + 5 + len) else {
            // Use what there is of this record to check the type early.
            if hs.is_empty() && buf.len() > pos + 5 && buf[pos + 5] != 0x01 {
                return Err(());
            }
            return Ok(None);
        };
        hs.extend_from_slice(frag);
        if hs[0] != 0x01 || hs.len() > MAX_HELLO + 4 {
            return Err(());
        }
        pos += 5 + len;
    }
}

/// Full handshake message length (header + body), once the header is there.
fn needed_handshake_len(hs: &[u8]) -> Option<usize> {
    let h = hs.get(..4)?;
    let n = (usize::from(h[1]) << 16) | (usize::from(h[2]) << 8) | usize::from(h[3]);
    Some(4 + n)
}

struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
    fn vec8(&mut self) -> Option<&'a [u8]> {
        let n = self.u8()?;
        self.take(usize::from(n))
    }
    fn vec16(&mut self) -> Option<&'a [u8]> {
        let n = self.u16()?;
        self.take(usize::from(n))
    }
}

/// The SNI host name in a ClientHello body (after the 4-byte handshake header).
fn find_sni(body: &[u8]) -> Option<Sni> {
    let mut c = Cursor(body);
    c.take(2 + 32)?; // client_version, random
    c.vec8()?; // session id
    c.vec16()?; // cipher suites
    c.vec8()?; // compression methods
    if c.0.is_empty() {
        return Some(Sni::NoName); // no extensions at all
    }
    let mut exts = Cursor(c.vec16()?);
    while !exts.0.is_empty() {
        let ty = exts.u16()?;
        let data = exts.vec16()?;
        if ty != 0 {
            continue;
        }
        let mut list = Cursor(Cursor(data).vec16()?);
        while !list.0.is_empty() {
            let name_type = list.u8()?;
            let name = list.vec16()?;
            if name_type == 0 {
                let host = core::str::from_utf8(name).ok()?;
                let ok = !host.is_empty()
                    && host.len() <= 253
                    && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
                return Some(if ok { Sni::Host(host.to_ascii_lowercase()) } else { Sni::NotTls });
            }
        }
        return Some(Sni::NoName);
    }
    Some(Sni::NoName)
}

/// Looks for the SNI host in the first bytes a client sent.
#[must_use]
pub fn parse_client_hello(buf: &[u8]) -> Sni {
    match handshake_bytes(buf) {
        Err(()) => Sni::NotTls,
        Ok(None) if buf.len() > MAX_HELLO + 1024 => Sni::NotTls,
        Ok(None) => Sni::NeedMore,
        Ok(Some(hs)) => find_sni(&hs[4..]).unwrap_or(Sni::NotTls),
    }
}
