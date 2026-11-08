//! Tests for `lanstash_domain::window`.

use lanstash_domain::window::Window;

const MIN: u64 = 60_000;
const DAY: u64 = 24 * 60 * MIN;

#[test]
fn a_window_within_a_day() {
    let w = Window::parse("00:00-04:00").unwrap();
    assert!(w.contains(0) && w.contains(3 * 60 * MIN + 59 * MIN));
    assert!(!w.contains(4 * 60 * MIN), "the end is outside");
    assert!(w.contains(5 * DAY + 60 * MIN), "any day");
    assert!(!w.contains(5 * DAY + 12 * 60 * MIN));
}

#[test]
fn a_window_past_midnight_and_the_whole_day() {
    let w = Window::parse("22:30-03:00").unwrap();
    assert!(w.contains(23 * 60 * MIN) && w.contains(MIN) && !w.contains(3 * 60 * MIN) && !w.contains(22 * 60 * MIN));
    let all = Window::parse("00:00-24:00").unwrap();
    assert!(all.contains(0) && all.contains(DAY - MIN));
}

#[test]
fn what_is_not_a_window() {
    for s in ["", "04:00", "4-5", "25:00-01:00", "01:60-02:00", "03:00-03:00", "24:00-01:00", "a:b-c:d"] {
        assert_eq!(Window::parse(s), None, "{s}");
    }
}
