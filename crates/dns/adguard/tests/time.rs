//! Tests for `lanstash_dns_adguard::time`.

use lanstash_dns_adguard::time::ms;

#[test]
fn reads_utc_times_with_and_without_fractions() {
    assert_eq!(ms("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(ms("2026-11-06T15:38:42.4098617Z"), Some(1_793_979_522_409));
    assert_eq!(ms("2000-02-29T12:00:00.5Z"), Some(951_825_600_500));
}

#[test]
fn applies_an_offset() {
    assert_eq!(ms("2026-11-06T19:38:42+04:00"), ms("2026-11-06T15:38:42Z"));
    assert_eq!(ms("2026-11-06T10:38:42-05:00"), ms("2026-11-06T15:38:42Z"));
}

#[test]
fn refuses_what_is_not_a_time() {
    for bad in
        ["", "2026-13-01T00:00:00Z", "2026-11-06", "2026-11-06T25:00:00Z", "1969-12-31T23:59:59Z", "x-y-zTa:b:cZ"]
    {
        assert_eq!(ms(bad), None, "{bad}");
    }
}
