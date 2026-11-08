//! Tests for `lanstash_domain::qr::rs`.

use lanstash_domain::qr::rs::{divisor, mul, remainder};

#[test]
fn the_field_multiplies_modulo_0x11d() {
    assert_eq!(mul(2, 0x80), 0x1D); // x * x^7 = x^8 = x^4 + x^3 + x^2 + 1
    assert_eq!(mul(0x53, 1), 0x53);
    assert_eq!(mul(0, 0xFF), 0);
    for a in 1..=255u8 {
        assert_eq!(mul(a, 3), mul(3, a));
    }
}

#[test]
fn a_codeword_with_its_correction_divides_evenly() {
    let data = b"lanstash keeps game downloads";
    for degree in [7, 10, 18, 26] {
        let div = divisor(degree);
        let mut all = data.to_vec();
        all.extend(remainder(data, &div));
        assert_eq!(remainder(&all, &div), vec![0; degree], "degree {degree}");
    }
}
