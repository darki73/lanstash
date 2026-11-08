//! Tests for `lanstash_domain::http::chunked`.

use lanstash_domain::http::chunked::ChunkedDecoder;

#[test]
fn chunked_decoding_in_arbitrary_pieces() {
    let wire = b"4;ext=1\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\nTrailer: x\r\n\r\nNEXT";
    for split in 1..wire.len() {
        let mut d = ChunkedDecoder::default();
        let mut out = Vec::new();
        let a = d.decode(&wire[..split], &mut out).unwrap();
        assert_eq!(a, split.min(wire.len() - 4), "split {split}");
        let b = d.decode(&wire[a..], &mut out).unwrap();
        assert!(d.is_done(), "split {split}");
        assert_eq!(a + b, wire.len() - 4);
        assert_eq!(out, b"Wikipedia in\r\n\r\nchunks.");
    }
    assert!(ChunkedDecoder::default().decode(b"zz\r\n", &mut Vec::new()).is_err());
    assert!(ChunkedDecoder::default().decode(b"2\r\nabX", &mut Vec::new()).is_err());
}
