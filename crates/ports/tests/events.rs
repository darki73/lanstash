use lanstash_ports::events::{Event, Events, NoEvents};

#[test]
fn no_events_takes_any_event() {
    let e: &dyn Events = &NoEvents;
    e.emit(&Event::KeyMismatch { key: "steam/x" });
    e.emit(&Event::Evicted { path: "steam/7/seg/00000.lcseg", bytes: 1, last_used_ms: 2 });
}
