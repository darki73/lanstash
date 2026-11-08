//! Tests for `lanstash_riot::platform`.

use lanstash_ports::platform::Platform;
use lanstash_riot::platform::RiotPlatform;

#[test]
fn riot_is_routing_only() {
    let p = RiotPlatform;
    assert_eq!(p.id(), "riot");
    assert!(p.art().is_none() && p.names().is_none() && p.catalog().is_none() && p.prefill().is_none());
}
