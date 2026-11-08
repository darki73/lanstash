//! The small answers of the request handler: a head without a body, a range past the end, and the
//! headers that say a response came through lanstash.

use lanstash_domain::http::reason_phrase;

use super::{ResponseHead, ResponseSink, SinkError};

/// An answer of `status` and `headers`, without a body.
pub(super) fn empty(sink: &mut dyn ResponseSink, status: u16, headers: &[(&str, &str)]) -> Result<(), SinkError> {
    sink.send_head(&ResponseHead { status, reason: reason_phrase(status), headers, content_length: 0 })
}

/// Adds what says a response came through lanstash: `X-LanStash` with `state` (`HIT`, `MISS`,
/// `PARTIAL`, `PASS`, `UPSTREAM`), and the two headers a lancache answers with, which tools
/// that look for a LAN cache read (`X-LanCache-Processed-By`, `X-Upstream-Cache-Status` in
/// nginx's words: `HIT`, `MISS`, `BYPASS`).
pub(crate) fn mark<'a>(headers: &mut Vec<(&'a str, &'a str)>, state: &'static str) {
    let nginx = match state {
        "HIT" => "HIT",
        "PASS" => "BYPASS",
        _ => "MISS",
    };
    headers.extend([
        ("X-LanStash", state),
        ("X-LanCache-Processed-By", "lanstash"),
        ("X-Upstream-Cache-Status", nginx),
    ]);
}

/// A range past an object's end: 416, saying how long the object is.
pub(super) fn unsatisfiable(sink: &mut dyn ResponseSink, total: u64) -> Result<(), SinkError> {
    let cr = format!("bytes */{total}");
    empty(sink, 416, &[("Content-Range", &cr), ("X-LanStash", "RANGE")])
}
