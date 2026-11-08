//! Tests for `lanstash_adapter_steam::link` (offline: the live path is in `wss.rs`).

use std::time::Duration;

use lanstash_adapter_steam::link::WssDialer;
use lanstash_ports::link::Dialer;

#[test]
fn a_refused_connection_is_an_error_not_a_hang() {
    let d: &dyn Dialer = &WssDialer { timeout: Duration::from_secs(2) };
    // Port 1 on loopback: nothing listens there.
    assert!(d.dial("127.0.0.1", 1, "/cmsocket/").is_err());
    assert!(d.get("127.0.0.1:1", "/").is_err());
}
