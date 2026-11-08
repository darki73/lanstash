//! A sink that counts how a client's response body went: the bytes the sink sent from the file
//! itself (`sendfile`) and those copied through lanstash, and whether writing to the client
//! failed (the client went away mid-response) rather than lanstash's own side.

use std::sync::atomic::Ordering;

use lanstash_ports::storage::FileRead;

use super::{ResponseHead, ResponseSink, ServeCounters, SinkError};

pub(super) struct Tally<'a> {
    inner: &'a mut dyn ResponseSink,
    counters: &'a ServeCounters,
    /// A write to the client failed.
    pub(super) gone: bool,
}

impl<'a> Tally<'a> {
    pub(super) fn new(inner: &'a mut dyn ResponseSink, counters: &'a ServeCounters) -> Self {
        Self { inner, counters, gone: false }
    }

    fn mark<T>(&mut self, r: Result<T, SinkError>) -> Result<T, SinkError> {
        if r.is_err() {
            self.gone = true;
        }
        r
    }
}

impl ResponseSink for Tally<'_> {
    fn send_head(&mut self, head: &ResponseHead<'_>) -> Result<(), SinkError> {
        let r = self.inner.send_head(head);
        self.mark(r)
    }
    fn send_stream_head(&mut self, status: u16, headers: &[(&str, &str)]) -> Result<(), SinkError> {
        let r = self.inner.send_stream_head(status, headers);
        self.mark(r)
    }
    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        let r = self.inner.send_body(data);
        self.mark(r)?;
        self.counters.copied_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(())
    }
    fn send_file(&mut self, file: &dyn FileRead, offset: u64, len: u64) -> Result<bool, SinkError> {
        let r = self.inner.send_file(file, offset, len);
        let sent = self.mark(r)?;
        if sent {
            self.counters.sendfile_bytes.fetch_add(len, Ordering::Relaxed);
        }
        Ok(sent)
    }
}
