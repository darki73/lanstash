//! `Systemd` on any machine: with or without systemd, a unit that does not
//! exist is neither installed, enabled nor active.

use std::time::Duration;

use lanstash_adapter_os::systemd::{Systemd, UnitState};

#[test]
fn waits_90_seconds_per_call_by_default() {
    assert_eq!(Systemd::default().timeout, Duration::from_secs(90));
}

#[test]
fn an_unknown_unit_is_not_there() {
    let s = Systemd { timeout: Duration::from_secs(20) };
    let state = s.state("lanstash-test-no-such-unit-0f3a.service");
    assert_eq!(state, UnitState { installed: false, enabled: false, active: false, main_pid: None });
    assert!(s.stop("lanstash-test-no-such-unit-0f3a.service").is_err());
}
