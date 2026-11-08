//! The shared state of one upstream download, read by every client waiting for
//! its bytes. The download thread lives in [`crate::fetch`].
//!
//! The download keeps the last `window` bytes in memory; a client further
//! behind reads them back from storage (they were written there before they
//! left the window). While a whole-body check is pending (Steam's
//! `x-content-crc`), the final byte is withheld, so a client can never
//! complete a body that fails it.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use lanstash_domain::extent::Span;
use lanstash_ports::storage::FileRead;

/// Windows of bytes a download keeps in memory when nothing can be read back from storage.
pub const MEMORY_WINDOWS: usize = 16;

/// What the origin's response turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadKind {
    /// Object bytes: `span` of an object of `total` bytes (`None`: length unknown, chunked).
    Content { span: Span, total: Option<u64> },
    /// The requested range starts past the end of the object.
    Unsatisfiable { total: u64 },
    /// Not object data (404, 403, 5xx, …): relayed to clients, never stored.
    Relay { status: u16, content_length: Option<u64> },
}

/// The response head of a download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlHead {
    pub kind: HeadKind,
    /// Stored headers (content) or relayed headers (relay), lower-case names.
    pub headers: Vec<(String, String)>,
}

/// Something a client can send next.
pub enum Piece {
    Bytes(Vec<u8>),
    /// Already written to storage: read `len` bytes at `offset`.
    Storage {
        reader: Arc<dyn FileRead>,
        offset: u64,
        len: u64,
    },
    /// Everything up to the end of the download was delivered.
    End,
    Failed(String),
}

impl std::fmt::Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bytes(b) => write!(f, "Bytes({})", b.len()),
            Self::Storage { offset, len, .. } => write!(f, "Storage {{ offset: {offset}, len: {len} }}"),
            Self::End => write!(f, "End"),
            Self::Failed(e) => write!(f, "Failed({e})"),
        }
    }
}

#[derive(Default)]
struct State {
    head: Option<Result<Arc<DlHead>, String>>,
    /// Bytes this download delivers (object offsets; for relays, body offsets).
    span: Span,
    /// End of the bytes received so far (absolute).
    received: u64,
    /// Bytes `buf_start..received`.
    buf: Vec<u8>,
    buf_start: u64,
    /// A whole-body check is pending: withhold the final byte until `end` is `Ok`.
    hold_last: bool,
    /// Where written bytes can be read back: (reader, file offset of object offset 0).
    back: Option<(Arc<dyn FileRead>, u64)>,
    end: Option<Result<(), String>>,
}

/// One download, shared by its thread and the clients reading it.
pub struct Download {
    /// The bytes asked for (`None`: the whole object, without `Range`).
    pub requested: Option<Span>,
    window: usize,
    state: Mutex<State>,
    changed: Condvar,
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.lock();
        f.debug_struct("Download")
            .field("requested", &self.requested)
            .field("span", &s.span)
            .field("received", &s.received)
            .finish_non_exhaustive()
    }
}

impl Download {
    #[must_use]
    pub fn new(requested: Option<Span>, window: usize) -> Self {
        Self { requested, window: window.max(64 * 1024), state: Mutex::default(), changed: Condvar::new() }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait<'a>(&self, g: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        self.changed.wait(g).unwrap_or_else(PoisonError::into_inner)
    }

    // ---- producer side: only the download thread (`crate::fetch`) calls these ----

    /// Publishes the response head. `span` is what this download will deliver.
    pub fn set_head(&self, head: DlHead, span: Span, hold_last: bool) {
        let mut s = self.lock();
        s.head = Some(Ok(Arc::new(head)));
        s.span = span;
        s.received = span.start;
        s.buf_start = span.start;
        s.hold_last = hold_last;
        drop(s);
        self.changed.notify_all();
    }

    /// Where bytes written so far can be read back from.
    pub fn set_read_back(&self, reader: Arc<dyn FileRead>, base: u64) {
        self.lock().back = Some((reader, base));
    }

    /// Storage failed: bytes must stay in memory for clients that are behind.
    pub fn clear_read_back(&self) {
        self.lock().back = None;
    }

    /// Appends received bytes (already written to storage, if any).
    pub fn push(&self, data: &[u8]) {
        let mut s = self.lock();
        s.buf.extend_from_slice(data);
        s.received += data.len() as u64;
        // Bytes leave memory once they can be read back; else only past a bound, so a download
        // that cannot be stored holds no more than that (a client further behind fails, and asks
        // again).
        let keep = if s.back.is_some() { self.window * 2 } else { self.window * MEMORY_WINDOWS };
        if s.buf.len() > keep {
            let drop_n = s.buf.len() - self.window;
            s.buf.drain(..drop_n);
            s.buf_start += drop_n as u64;
        }
        drop(s);
        self.changed.notify_all();
    }

    /// Ends the download (before any head: the request failed).
    pub fn finish(&self, result: Result<(), String>) {
        let mut s = self.lock();
        if s.head.is_none() {
            s.head = Some(Err(result.clone().err().unwrap_or_else(|| "no response".into())));
        }
        s.end = Some(result);
        drop(s);
        self.changed.notify_all();
    }

    // ---- client side ----

    /// Waits for the response head.
    ///
    /// # Errors
    /// The request failed before a head arrived.
    pub fn wait_head(&self) -> Result<Arc<DlHead>, String> {
        let mut s = self.lock();
        loop {
            if let Some(h) = &s.head {
                return h.clone();
            }
            s = self.wait(s);
        }
    }

    /// The delivered span, once the head is known.
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        let s = self.lock();
        matches!(s.head, Some(Ok(_))).then_some(s.span)
    }

    /// (span once known, end of the bytes received so far).
    #[must_use]
    pub fn progress(&self) -> (Option<Span>, u64) {
        let s = self.lock();
        (matches!(s.head, Some(Ok(_))).then_some(s.span), s.received)
    }

    /// Whether the download finished (successfully or not).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.lock().end.is_some()
    }

    /// Whether the origin answered with a server error (5xx) this download relays: a passing
    /// error, never the answer to a request that comes after it.
    #[must_use]
    pub fn relays_server_error(&self) -> bool {
        matches!(&self.lock().head, Some(Ok(h)) if matches!(h.kind, HeadKind::Relay { status, .. } if status >= 500))
    }

    /// Waits for the end and returns the number of bytes received (for bodies
    /// of unknown length).
    ///
    /// # Errors
    /// The download failed.
    pub fn wait_end(&self) -> Result<u64, String> {
        let mut s = self.lock();
        loop {
            match &s.end {
                Some(Ok(())) => return Ok(s.received - s.span.start),
                Some(Err(e)) => return Err(e.clone()),
                None => s = self.wait(s),
            }
        }
    }

    /// The next bytes at `pos` (at most `max`), waiting until they arrived.
    pub fn next(&self, pos: u64, max: usize) -> Piece {
        let mut s = self.lock();
        loop {
            match &s.head {
                None => {
                    s = self.wait(s);
                    continue;
                }
                Some(Err(e)) => return Piece::Failed(e.clone()),
                Some(Ok(_)) => {}
            }
            let ended_ok = matches!(s.end, Some(Ok(())));
            let failed = matches!(s.end, Some(Err(_)));
            // `span.end` is u64::MAX for bodies of unknown length; then `received` marks the end.
            if pos >= s.span.end || (ended_ok && pos >= s.received) {
                return Piece::End;
            }
            let readable = if failed && s.hold_last {
                s.span.start // the body failed its check: nothing of it is trusted
            } else if s.hold_last && !ended_ok {
                s.received.min(s.span.end.saturating_sub(1))
            } else {
                s.received
            };
            if pos < readable {
                let end = readable.min(pos.saturating_add(max as u64));
                if pos >= s.buf_start {
                    let a = usize::try_from(pos - s.buf_start).unwrap_or(usize::MAX);
                    let b = usize::try_from(end - s.buf_start).unwrap_or(usize::MAX);
                    return Piece::Bytes(s.buf[a..b].to_vec());
                }
                return match &s.back {
                    Some((reader, base)) => Piece::Storage {
                        reader: Arc::clone(reader),
                        offset: base + pos,
                        len: s.buf_start.min(end) - pos,
                    },
                    None => Piece::Failed("client fell behind the download".into()),
                };
            }
            if let Some(Err(e)) = &s.end {
                return Piece::Failed(e.clone());
            }
            s = self.wait(s);
        }
    }
}
