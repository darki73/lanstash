//! The body of an object response: stored parts read from disk one piece
//! ahead of the socket, missing parts from downloads, failed downloads
//! replaced from the client position.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use lanstash_domain::extent::Span;
use lanstash_domain::segment::record::RecordHeader;
use lanstash_ports::storage::{FileRead, StoreError};
use lanstash_store::index::SegId;
use lanstash_store::index::{Lookup, Source};

use crate::download::{Download, Piece};
use crate::fetch::Cover;

use super::{Cache, ConnState, Plan, Prefetch, ResponseSink, SinkError};

impl Cache {
    /// Walks the planned span: stored parts from disk, the rest from downloads.
    pub(super) fn send_body(
        &self,
        plan: &mut Plan,
        conn: &mut ConnState,
        sink: &mut dyn ResponseSink,
    ) -> Result<(), SinkError> {
        let mut pos = plan.span.start;
        let end = plan.span.end;
        let mut retries = 0;
        let mut idle_rounds = 0;
        while pos < end {
            if let Some((span, src)) = plan.view.as_ref().and_then(|o| o.at(pos)).cloned() {
                let to = span.end.min(end);
                self.send_stored(plan, &span, &src, pos, to, conn, sink)?;
                pos = to;
                idle_rounds = 0;
                continue;
            }
            // A download the head came from may already cover this position.
            if let Some(d) = plan.first_download.take()
                && d.span().is_some_and(|s| s.contains(pos))
            {
                pos = self.send_download(&d, pos, end, sink, &mut retries)?;
                continue;
            }
            plan.view = match plan.obj.look(&self.store) {
                Lookup::Found(o) => Some(o),
                Lookup::Unusable => return Err(SinkError("object became unusable".into())),
                Lookup::Missing => None,
            };
            if plan.view.as_ref().is_some_and(|o| o.at(pos).is_some()) {
                continue;
            }
            let Some(f) = &self.fetcher else { return Err(SinkError("not stored".into())) };
            match f.cover(&plan.obj, pos, end, plan.total) {
                Cover::Unusable => return Err(SinkError("object became unusable".into())),
                Cover::Download(d) => {
                    idle_rounds = 0;
                    pos = self.send_download(&d, pos, end, sink, &mut retries)?;
                }
                Cover::Stored => {
                    idle_rounds += 1;
                    if idle_rounds > 100 {
                        return Err(SinkError(format!("no source for byte {pos} of {}", plan.obj.key)));
                    }
                    std::thread::yield_now();
                }
            }
        }
        Ok(())
    }

    /// Sends from a download until it ends or `end`; returns the new position.
    /// A failed download, or one that ends without delivering a byte, costs one retry and
    /// returns where it stopped.
    pub(super) fn send_download(
        &self,
        d: &Download,
        mut pos: u64,
        end: u64,
        sink: &mut dyn ResponseSink,
        retries: &mut usize,
    ) -> Result<u64, SinkError> {
        let start = pos;
        while pos < end {
            let max = usize::try_from((end - pos).min(self.opts.piece_bytes as u64)).unwrap_or(4096);
            match d.next(pos, max) {
                Piece::Bytes(b) => {
                    sink.send_body(&b)?;
                    pos += b.len() as u64;
                }
                Piece::Storage { reader, offset, len } => {
                    let n = len.min(end - pos);
                    stream_file(&reader, offset, n, self.opts.piece_bytes, sink)?;
                    pos += n;
                }
                Piece::End if pos == start => {
                    *retries += 1;
                    self.counters.retries.fetch_add(1, Ordering::Relaxed);
                    if *retries > self.opts.retries {
                        return Err(SinkError(format!("download ended before byte {pos}")));
                    }
                    return Ok(pos);
                }
                Piece::End => return Ok(pos),
                Piece::Failed(e) => {
                    *retries += 1;
                    self.counters.retries.fetch_add(1, Ordering::Relaxed);
                    if *retries > self.opts.retries {
                        return Err(SinkError(format!("download failed: {e}")));
                    }
                    return Ok(pos);
                }
            }
        }
        Ok(pos)
    }

    /// Sends `from..to` of one stored extent.
    #[allow(clippy::too_many_arguments)]
    fn send_stored(
        &self,
        plan: &mut Plan,
        span: &Span,
        src: &Source,
        from: u64,
        to: u64,
        conn: &mut ConnState,
        sink: &mut dyn ResponseSink,
    ) -> Result<(), SinkError> {
        match src {
            Source::File { file } => {
                let reader = file.reader().map_err(|e| SinkError(format!("storage: {e}")))?;
                stream_file(&reader, from, to - from, self.opts.piece_bytes, sink)
            }
            Source::Record { seg, reader, offset, header_len } => {
                let at = (*seg, *offset);
                let body_off = offset + u64::from(*header_len);
                let prefetched = self.take_prefetch(conn, at);
                let sequential = conn.last.is_some_and(|(s, o)| {
                    self.store.index().next_record(s, o).is_some_and(|(next, ..)| s == *seg && next == *offset)
                });
                if self.opts.prefetch && (sequential || prefetched.is_some()) {
                    conn.prefetch = self.spawn_prefetch(*seg, *offset);
                }
                conn.last = Some(at);
                let key = &plan.obj.key;
                // From memory: a prefetched record, or the one read for the head.
                let mem = match prefetched {
                    Some(b) => Some((b, *header_len)),
                    None => plan.first_record.take().filter(|(s, o, ..)| (*s, *o) == at).map(|(_, _, b, h)| (b, h)),
                };
                if let Some((bytes, h)) = mem {
                    let h = h as usize;
                    let ok = RecordHeader::decode(&bytes[..h.min(bytes.len())])
                        .is_ok_and(|r| &r.key == key && r.body_len == span.len());
                    if !ok {
                        self.key_mismatch(key);
                        return Err(SinkError("record belongs to another key".into()));
                    }
                    // `bytes` holds the header and the body from 0.
                    let have = (bytes.len() - h) as u64;
                    let mut pos = from;
                    if pos < have {
                        let upto = to.min(have);
                        sink.send_body(&bytes[h + pos as usize..h + upto as usize])?;
                        pos = upto;
                    }
                    if pos < to {
                        stream_file(reader, body_off + pos, to - pos, self.opts.piece_bytes, sink)?;
                    }
                    return Ok(());
                }
                match record_header(&**reader, *offset, *header_len, key, span.len()) {
                    Ok(Some(_)) => stream_file(reader, body_off + from, to - from, self.opts.piece_bytes, sink),
                    Ok(None) => {
                        self.key_mismatch(key);
                        Err(SinkError("record belongs to another key".into()))
                    }
                    Err(e) => Err(SinkError(format!("storage: {e}"))),
                }
            }
        }
    }

    fn take_prefetch(&self, conn: &mut ConnState, at: (SegId, u64)) -> Option<Vec<u8>> {
        match conn.prefetch.take() {
            Some(p) if p.at == at => {
                let r = p.handle.join().ok().and_then(Result::ok);
                if r.is_some() {
                    self.counters.prefetch_used.fetch_add(1, Ordering::Relaxed);
                }
                r
            }
            Some(_) => {
                self.counters.prefetch_wasted.fetch_add(1, Ordering::Relaxed);
                None
            }
            None => None,
        }
    }

    /// Reads the record after `(seg, offset)` in the background.
    fn spawn_prefetch(&self, seg: SegId, offset: u64) -> Option<Prefetch> {
        let (next, header_len, body_len, reader) = self.store.index().next_record(seg, offset)?;
        let len = usize::try_from(u64::from(header_len) + body_len).ok()?;
        let handle = std::thread::Builder::new()
            .name("lanstash-prefetch".into())
            .stack_size(64 * 1024)
            .spawn(move || {
                let mut buf = vec![0u8; len];
                reader.read_exact_at(next, &mut buf).map(|()| buf)
            })
            .ok()?;
        Some(Prefetch { at: (seg, next), handle })
    }
}

/// Reads a whole record header at `offset` and checks it belongs to `key` with `body_len`.
pub(super) fn record_header(
    reader: &dyn FileRead,
    offset: u64,
    header_len: u32,
    key: &str,
    body_len: u64,
) -> Result<Option<RecordHeader>, StoreError> {
    let mut b = vec![0u8; header_len as usize];
    reader.read_exact_at(offset, &mut b)?;
    Ok(RecordHeader::decode(&b).ok().filter(|h| h.key == key && h.body_len == body_len))
}

/// Sends `len` bytes from `reader` at `offset`: by the sink itself where it can do it
/// without copying them through lanstash ([`ResponseSink::send_file`]), else read piece by
/// piece, one piece ahead.
///
/// The pieces' buffers go back to the reader once sent and are filled again: a fresh buffer
/// per piece costs the kernel a page fault and zeroing for every page of it.
pub(super) fn stream_file(
    reader: &Arc<dyn FileRead>,
    offset: u64,
    len: u64,
    piece: usize,
    sink: &mut dyn ResponseSink,
) -> Result<(), SinkError> {
    if len > 0 && sink.send_file(&**reader, offset, len)? {
        return Ok(());
    }
    let piece = piece.max(4096) as u64;
    if len <= piece {
        let mut buf = vec![0u8; usize::try_from(len).unwrap_or(0)];
        reader.read_exact_at(offset, &mut buf).map_err(|e| SinkError(format!("storage: {e}")))?;
        return sink.send_body(&buf);
    }
    std::thread::scope(|s| {
        // Bounded to 2 pieces in flight: memory per connection stays small.
        let (tx, rx) = mpsc::sync_channel::<Result<Vec<u8>, StoreError>>(2);
        // The buffers sent, back to be filled again.
        let (back, spare) = mpsc::channel::<Vec<u8>>();
        let reader = Arc::clone(reader);
        // A small stack: it only reads into the buffers (a body over one piece spawns one; a
        // Steam chunk of a megabyte does).
        let spawned =
            std::thread::Builder::new().name("lanstash-read".into()).stack_size(64 * 1024).spawn_scoped(s, move || {
                let mut pos = 0;
                while pos < len {
                    let n = usize::try_from(piece.min(len - pos)).unwrap_or(0);
                    let mut buf = spare.try_recv().unwrap_or_default();
                    // A buffer back from the sender is already this long (the last piece is shorter):
                    // only a new one is zeroed.
                    buf.resize(n, 0);
                    let r = reader.read_exact_at(offset + pos, &mut buf).map(|()| buf);
                    let failed = r.is_err();
                    if tx.send(r).is_err() || failed {
                        return;
                    }
                    pos += n as u64;
                }
            });
        if let Err(e) = spawned {
            return Err(SinkError(format!("no reader thread: {e}")));
        }
        for part in rx {
            let part = part.map_err(|e| SinkError(format!("storage failed mid-body: {e}")))?;
            sink.send_body(&part)?;
            // The reader may be done already: then the buffer is simply dropped.
            let _ = back.send(part);
        }
        Ok(())
    })
}
