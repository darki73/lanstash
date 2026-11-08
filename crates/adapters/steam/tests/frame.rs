//! WebSocket frames: a client's frames masked, the server's read back, in parts.

use lanstash_adapter_steam::frame::{BINARY, Frame, PING, decode, encode};

#[test]
fn a_masked_frame_reads_back() {
    for n in [0, 1, 125, 126, 300, 70_000] {
        let payload: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
        let f = encode(BINARY, &payload, [1, 2, 3, 4]);
        let (got, used) = decode(&f, 1 << 20).unwrap().unwrap();
        assert_eq!((got, used), (Frame { fin: true, opcode: BINARY, payload }, f.len()), "{n} bytes");
        assert_eq!(decode(&f[..f.len() - 1], 1 << 20).unwrap(), None, "not all there yet");
    }
}

#[test]
fn a_server_frame_unmasked_and_too_large_ones() {
    // RFC 6455 section 5.7: an unmasked text "Hello", then a ping.
    let mut buf = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o', 0x89, 0x00];
    let (f, used) = decode(&buf, 100).unwrap().unwrap();
    assert_eq!((f.payload.as_slice(), used), (&b"Hello"[..], 7));
    buf.drain(..used);
    assert_eq!(decode(&buf, 100).unwrap().unwrap().0.opcode, PING);
    assert!(decode(&encode(BINARY, &[0; 200], [0; 4]), 100).is_err());
}
