//! WebSocket frames (RFC 6455 section 5), the client's side: what it sends is
//! masked; what the server sends is not. Pure: bytes in, bytes out.

/// Frame opcodes this client meets.
pub const CONTINUATION: u8 = 0x0;
pub const TEXT: u8 = 0x1;
pub const BINARY: u8 = 0x2;
pub const CLOSE: u8 = 0x8;
pub const PING: u8 = 0x9;
pub const PONG: u8 = 0xA;

/// A client frame (final, masked with `mask`) of `opcode` carrying `payload`.
#[must_use]
pub fn encode(opcode: u8, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut f = vec![0x80 | (opcode & 0x0F)];
    let n = payload.len();
    if n < 126 {
        f.push(0x80 | n as u8);
    } else if let Ok(n) = u16::try_from(n) {
        f.push(0x80 | 126);
        f.extend_from_slice(&n.to_be_bytes());
    } else {
        f.push(0x80 | 127);
        f.extend_from_slice(&(n as u64).to_be_bytes());
    }
    f.extend_from_slice(&mask);
    f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    f
}

/// One frame from the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub fin: bool,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

/// The first frame in `buf` and how many bytes it took; `Ok(None)` until it
/// is all there.
///
/// # Errors
/// A frame longer than `max`.
pub fn decode(buf: &[u8], max: usize) -> Result<Option<(Frame, usize)>, String> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let (fin, opcode, masked) = (buf[0] & 0x80 != 0, buf[0] & 0x0F, buf[1] & 0x80 != 0);
    let (len, mut at) = match buf[1] & 0x7F {
        126 if buf.len() >= 4 => (u64::from(u16::from_be_bytes([buf[2], buf[3]])), 4),
        127 if buf.len() >= 10 => (u64::from_be_bytes(buf[2..10].try_into().unwrap_or([0; 8])), 10),
        126 | 127 => return Ok(None),
        n => (u64::from(n), 2),
    };
    let len = usize::try_from(len).ok().filter(|l| *l <= max).ok_or(format!("a frame of {len} bytes"))?;
    let mask = if masked {
        let Some(m) = buf.get(at..at + 4) else { return Ok(None) };
        at += 4;
        Some([m[0], m[1], m[2], m[3]])
    } else {
        None
    };
    let Some(body) = buf.get(at..at + len) else { return Ok(None) };
    let payload = match mask {
        Some(m) => body.iter().enumerate().map(|(i, b)| b ^ m[i % 4]).collect(),
        None => body.to_vec(),
    };
    Ok(Some((Frame { fin, opcode, payload }, at + len)))
}
