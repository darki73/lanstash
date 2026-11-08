//! Tests for `lanstash_adapter_os::pause`.

use std::time::{Duration, Instant};

use lanstash_adapter_os::pause::Sleep;
use lanstash_ports::clock::Pause;

#[test]
fn a_pause_takes_about_as_long_as_asked() {
    let start = Instant::now();
    Sleep.pause_ms(30);
    assert!(start.elapsed() >= Duration::from_millis(30));
}
