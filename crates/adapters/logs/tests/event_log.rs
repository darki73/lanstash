//! Event log lines.

use lanstash_adapter_logs::event_log::format_event;
use lanstash_domain::extent::Span;
use lanstash_ports::events::Event;

#[test]
fn one_line_per_event() {
    let e =
        Event::DownloadFailed { key: "blizzard/tpr/x", host: "eu.cdn", span: Some(Span::new(0, 10)), error: "reset" };
    assert_eq!(format_event(7, &e), r#"7 download-failed host=eu.cdn key=blizzard/tpr/x span=0-10 error="reset""#);
    assert_eq!(format_event(8, &Event::KeyMismatch { key: "k" }), "8 key-mismatch key=k");
    let s = Event::StoreFailed { key: "k", error: "disk\nfull" };
    assert!(!format_event(9, &s).contains('\n'), "a line stays one line");
}
