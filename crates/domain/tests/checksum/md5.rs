//! Tests for `lanstash_domain::checksum::md5`.

use lanstash_domain::checksum::md5::*;

fn hex(data: &[u8]) -> std::string::String {
    std::string::String::from_utf8(md5_hex(data).to_vec()).unwrap()
}

#[test]
fn rfc1321_test_suite() {
    assert_eq!(hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(hex(b"a"), "0cc175b9c0f1b6a831c399e269772661");
    assert_eq!(hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(hex(b"message digest"), "f96b697d7cb7938d525a2f31aaf161d0");
    assert_eq!(hex(b"abcdefghijklmnopqrstuvwxyz"), "c3fcd3d76192e4007dfb496cca67e13b");
    assert_eq!(
        hex(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"),
        "d174ab98d277d9f5a5611c2c9f419d9f"
    );
    assert_eq!(
        hex(b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"),
        "57edf4a22be3c955ac49da2e2107b67a"
    );
}

#[test]
fn nginx_cache_file_name() {
    // nginx stores this key in 7a/00/009e7e74e5f962fcbd151381e957007a.
    let key = b"steam/depot/2357571/chunk/9de1d873a59f1c1d108f63112ffe8903dc4b5ae1bytes=0-1048575";
    assert_eq!(hex(key), "009e7e74e5f962fcbd151381e957007a");
}

#[test]
fn in_parts_as_whole() {
    let data: std::vec::Vec<u8> = (0..1000u32).map(|i| (i * 31 % 251) as u8).collect();
    for part in [1, 3, 63, 64, 65, 127, 128, 999, 1000] {
        let mut m = Md5::new();
        for p in data.chunks(part) {
            m.update(p);
        }
        assert_eq!(m.finish(), md5(&data), "parts of {part}");
    }
    assert_eq!(Md5::new().finish(), md5(b""));
}
