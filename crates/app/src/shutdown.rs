//! Use case: a clean stop. Seals every open segment and saves the last-use
//! times, so the next start reads only footers (no scan) and loses no
//! recency. In two steps, so the caller decides how long to wait for records
//! still being written: [`Shutdown::begin`], then [`Shutdown::finish`].
//! Whatever is still open then is left to crash recovery.

use std::sync::Arc;

use lanstash_store::game::ActiveSegment;
use lanstash_store::store::Store;

/// What a clean stop did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShutdownReport {
    pub segments_sealed: usize,
    /// Segments still being written at the end.
    pub segments_open: usize,
    /// Games whose last-use times could not be saved: (game, error).
    pub recency_failed: Vec<(String, String)>,
}

/// A stop in progress.
pub struct Shutdown {
    store: Arc<Store>,
    closing: Vec<Arc<ActiveSegment>>,
}

impl std::fmt::Debug for Shutdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shutdown").field("closing", &self.closing.len()).finish_non_exhaustive()
    }
}

impl Shutdown {
    /// Closes every active segment: no new records; each seals as soon as the
    /// records being written into it are done.
    #[must_use]
    pub fn begin(store: Arc<Store>) -> Self {
        let closing = store.close_segments();
        Self { store, closing }
    }

    /// Segments not sealed yet.
    #[must_use]
    pub fn open(&self) -> usize {
        self.closing.iter().filter(|s| !s.appender.is_sealed()).count()
    }

    /// Saves the last-use times and reports.
    #[must_use]
    pub fn finish(self) -> ShutdownReport {
        let open = self.open();
        let recency_failed = self.store.save_recency().into_iter().map(|(g, e)| (g.path(), e.to_string())).collect();
        ShutdownReport { segments_sealed: self.closing.len() - open, segments_open: open, recency_failed }
    }
}
