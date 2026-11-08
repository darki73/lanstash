//! The status event stream (`GET /api/v1/events`): one snapshot an interval, for as long as the
//! page that asked stays open, at most [`MAX_STREAMS`] at once and [`PER_ADDRESS`] from one
//! address. Each event is what its viewer may
//! see ([`StreamView`]): hidden games masked, the clients unnamed for a guest. Every
//! [`RECHECK_EVENTS`] events the stream looks whether its viewer may still see as much (its
//! session ended, or its role changed): if not it ends, and the page, reconnecting at once, gets
//! what it may see now.

use std::net::IpAddr;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use lanstash_app::serve::{ResponseSink, SinkError};

use super::{AdminApi, MAX_STREAMS};

/// How many events an event stream sends between two looks at whether its viewer may still see them.
pub const RECHECK_EVENTS: usize = 10;

/// The most event streams one address may hold at once (a few pages open), so that no one holds
/// them all.
pub const PER_ADDRESS: usize = 3;

/// What an event stream's viewer may see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamView {
    /// Hidden games are masked (the viewer may not hide games).
    pub masked: bool,
    /// The clients' addresses and names are left out (no one is signed in).
    pub guest: bool,
}

impl AdminApi {
    /// Sends one event per interval until the viewer goes away or `still` says it may no longer see
    /// as `view` does; `None` when [`MAX_STREAMS`] are open already, or [`PER_ADDRESS`] from the
    /// viewer's address `from` (the caller answers 503).
    pub(super) fn stream(
        &self,
        sink: &mut dyn ResponseSink,
        (view, reader): (StreamView, Option<&str>),
        from: Option<IpAddr>,
        still: &dyn Fn() -> bool,
    ) -> Option<Result<(), SinkError>> {
        if let Some(a) = from {
            let mut held = self.streams_from.lock().unwrap_or_else(PoisonError::into_inner);
            let n = held.entry(a).or_insert(0);
            if *n >= PER_ADDRESS {
                return None;
            }
            *n += 1;
        }
        let r = if self.streams.fetch_add(1, Ordering::SeqCst) >= MAX_STREAMS {
            None
        } else {
            Some(self.stream_on(sink, (view, reader), still))
        };
        self.streams.fetch_sub(1, Ordering::SeqCst);
        if let Some(a) = from {
            let mut held = self.streams_from.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(n) = held.get_mut(&a) {
                *n -= 1;
                if *n == 0 {
                    held.remove(&a);
                }
            }
        }
        r
    }

    fn stream_on(
        &self,
        sink: &mut dyn ResponseSink,
        (view, reader): (StreamView, Option<&str>),
        still: &dyn Fn() -> bool,
    ) -> Result<(), SinkError> {
        sink.send_stream_head(200, &[("Content-Type", "text/event-stream"), ("Cache-Control", "no-store")])?;
        // Reconnect quickly after a restart of lanstash.
        sink.send_body(b"retry: 2000\n\n")?;
        let mut prev = None;
        for n in 1usize.. {
            if n % RECHECK_EVENTS == 0 && !still() {
                return Ok(());
            }
            let mut snapshot = self.snapshot(&mut prev);
            if let Some(m) = self.viewer_mask(reader).filter(|_| view.masked) {
                snapshot = m.json(&snapshot, None, false);
            }
            if view.guest {
                snapshot = crate::anon::without_clients(&snapshot, &self.pseudonyms);
            }
            let event = format!("data: {snapshot}\n\n");
            if sink.send_body(event.as_bytes()).is_err() {
                return Ok(()); // the viewer closed the page
            }
            std::thread::sleep(self.interval);
        }
        Ok(())
    }
}
