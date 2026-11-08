//! Tests for `lanstash_domain::checksum::crc`.

use lanstash_domain::checksum::crc::*;

/// Bit-by-bit reference implementation, independent of the tables.
fn reference(poly: u32, data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ poly } else { crc >> 1 };
        }
    }
    !crc
}

#[test]
fn standard_check_values() {
    // CRC catalogue check values over "123456789".
    assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn rfc3720_vectors() {
    // RFC 3720 (iSCSI), appendix B.4.
    assert_eq!(crc32c(&[0u8; 32]), 0x8A91_36AA);
    assert_eq!(crc32c(&[0xFFu8; 32]), 0x62A8_AB43);
    let inc: Vec<u8> = (0u8..32).collect();
    assert_eq!(crc32c(&inc), 0x46DD_794E);
    let dec: Vec<u8> = (0u8..32).rev().collect();
    assert_eq!(crc32c(&dec), 0x113F_DB5C);
}

#[test]
fn empty_input() {
    assert_eq!(crc32c(&[]), 0);
    assert_eq!(crc32(&[]), 0);
}

#[test]
fn matches_reference_on_many_lengths_and_splits() {
    let data: Vec<u8> = (0..4099u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
    for len in [0, 1, 7, 8, 9, 15, 16, 63, 64, 65, 1000, 4099] {
        let d = &data[..len];
        assert_eq!(crc32c(d), reference(0x82F6_3B78, d), "crc32c len {len}");
        assert_eq!(crc32(d), reference(0xEDB8_8320, d), "crc32 len {len}");
        for split in [0, 1, 3, len / 2, len].into_iter().filter(|&s| s <= len) {
            let mut c = Crc32c::new();
            c.update(&d[..split]);
            c.update(&d[split..]);
            assert_eq!(c.finish(), reference(0x82F6_3B78, d), "len {len} split {split}");
        }
    }
}
