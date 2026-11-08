use lanstash_ports::clock::{Clock, FixedClock};

#[test]
fn a_fixed_clock_never_moves() {
    let c: &dyn Clock = &FixedClock(1_700_000_000_000);
    assert_eq!(c.now_ms(), 1_700_000_000_000);
    assert_eq!(c.now_ms(), 1_700_000_000_000);
}

#[test]
fn no_pause_returns_at_once() {
    use lanstash_ports::clock::{NoPause, Pause};
    let p: &dyn Pause = &NoPause;
    p.pause_ms(60_000);
}
