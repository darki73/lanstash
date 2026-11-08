//! Use case: delete one game from the stash on request (a game nobody wants
//! kept), as a job on a thread of its own, one at a time, with its progress
//! readable meanwhile ([`GameDeleter::status`]). Everything the game holds
//! goes the way eviction deletes ([`lanstash_store::evict`]): every segment
//! and object file of its directory, each announced as evicted, then the
//! journals compacted. The segment the game is writing into is closed first,
//! and sealed once the records being written into it are done. A prune may
//! also compact segments ([`PruneItem::Compact`]): the objects still wanted
//! are copied out, then the segment goes.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::Clock;
use lanstash_ports::events::{Event, Events};
use lanstash_ports::platform::PruneItem;
use lanstash_store::evict::EvictItem;
use lanstash_store::layout::GameDir;
use lanstash_store::store::Store;

/// Where a deletion stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteState {
    Running,
    Done,
}

impl DeleteState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
        }
    }
}

/// The last deletion, running or finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteStatus {
    /// The service (platform) and the game directory.
    pub platform: String,
    pub game: String,
    pub state: DeleteState,
    /// Segments and object files to delete, and done so far.
    pub items_total: u64,
    pub items_done: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
    /// What could not be deleted: (path, why).
    pub failed: Vec<(String, String)>,
    pub started_ms: u64,
    pub finished_ms: Option<u64>,
    /// Only some object files went (old builds'), not the whole game.
    pub partial: bool,
    /// Bytes copied out of compacted segments (kept; the rest of them went).
    pub bytes_kept: u64,
}

/// Why a deletion did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteError {
    /// Another deletion is running.
    Busy,
    /// The thread could not be started.
    Spawn(String),
}

/// A segment to compact: its directory, sequence number, and the keys to keep.
type Compaction = (GameDir, u32, BTreeSet<String>);

/// What a partial deletion takes: these items, and these segments compacted.
struct Scope {
    only: Vec<(GameDir, EvictItem)>,
    compact: Vec<Compaction>,
}

struct Shared {
    store: Arc<Store>,
    events: Arc<dyn Events>,
    clock: Arc<dyn Clock>,
    status: Mutex<Option<DeleteStatus>>,
}

/// Deletes games from a store, one at a time, announcing what goes.
pub struct GameDeleter {
    shared: Arc<Shared>,
    /// Games deleted whole since they were last taken ([`GameDeleter::take_deleted`]).
    deleted: Mutex<Vec<(String, String)>>,
}

impl std::fmt::Debug for GameDeleter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameDeleter").finish_non_exhaustive()
    }
}

impl GameDeleter {
    #[must_use]
    pub fn new(store: Arc<Store>, events: Arc<dyn Events>, clock: Arc<dyn Clock>) -> Self {
        Self { shared: Arc::new(Shared { store, events, clock, status: Mutex::new(None) }), deleted: Mutex::default() }
    }

    /// The games deleted whole (as `(platform, game)`) since the last call: what keeps games
    /// current must stop doing so for them (a game deleted by hand is not wanted back).
    #[must_use]
    pub fn take_deleted(&self) -> Vec<(String, String)> {
        std::mem::take(&mut *self.deleted.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Starts deleting what the stash holds of `service`'s `game` (its game directory).
    ///
    /// # Errors
    /// [`DeleteError::Busy`] while another deletion runs.
    pub fn start(&self, service: &str, game: &str) -> Result<(), DeleteError> {
        self.start_scoped(service, game, vec![game.to_owned()], None)
    }

    /// Starts deleting a game made of several game directories (a Steam app's
    /// depots), as one deletion called `game`.
    ///
    /// # Errors
    /// [`DeleteError::Busy`] while another deletion runs.
    pub fn start_parts(&self, service: &str, game: &str, parts: Vec<String>) -> Result<(), DeleteError> {
        self.start_scoped(service, game, parts, None)
    }

    /// Starts deleting only object files `files` (journal numbers) of `service`'s
    /// `game`: what it no longer needs ([`DeleteStatus::partial`]).
    ///
    /// # Errors
    /// [`DeleteError::Busy`] while another deletion runs.
    pub fn start_files(&self, service: &str, game: &str, files: Vec<u32>) -> Result<(), DeleteError> {
        let items = files.into_iter().map(|no| PruneItem::File { dir: game.to_owned(), no }).collect();
        self.start_items(service, game, items)
    }

    /// Starts deleting only `items` (object files and segments of one or more of
    /// `service`'s directories), as one partial deletion called `game`.
    ///
    /// # Errors
    /// [`DeleteError::Busy`] while another deletion runs.
    pub fn start_items(&self, service: &str, game: &str, items: Vec<PruneItem>) -> Result<(), DeleteError> {
        let mut only: Vec<(GameDir, EvictItem)> = Vec::new();
        let mut compact: Vec<Compaction> = Vec::new();
        for i in items {
            match i {
                PruneItem::File { dir, no } => only.push((GameDir::new(service, &dir), EvictItem::File(no))),
                PruneItem::Segment { dir, seq } => only.push((GameDir::new(service, &dir), EvictItem::Segment(seq))),
                PruneItem::Compact { dir, seq, keep } => {
                    compact.push((GameDir::new(service, &dir), seq, keep.into_iter().collect()));
                }
            }
        }
        let mut dirs: Vec<String> = only.iter().map(|(d, _)| d.game.clone()).collect();
        dirs.extend(compact.iter().map(|(d, ..)| d.game.clone()));
        dirs.sort();
        dirs.dedup();
        if dirs.is_empty() {
            dirs.push(game.to_owned());
        }
        self.start_scoped(service, game, dirs, Some(Scope { only, compact }))
    }

    fn start_scoped(
        &self,
        service: &str,
        game: &str,
        parts: Vec<String>,
        files: Option<Scope>,
    ) -> Result<(), DeleteError> {
        {
            let mut st = self.shared.lock();
            if st.as_ref().is_some_and(|s| s.state == DeleteState::Running) {
                return Err(DeleteError::Busy);
            }
            *st = Some(DeleteStatus {
                platform: service.to_owned(),
                game: game.to_owned(),
                state: DeleteState::Running,
                items_total: 0,
                items_done: 0,
                bytes_total: 0,
                bytes_done: 0,
                failed: Vec::new(),
                started_ms: self.shared.clock.now_ms(),
                finished_ms: None,
                partial: files.is_some(),
                bytes_kept: 0,
            });
        }
        if files.is_none() {
            self.deleted.lock().unwrap_or_else(PoisonError::into_inner).push((service.to_owned(), game.to_owned()));
        }
        let shared = Arc::clone(&self.shared);
        let dirs: Vec<GameDir> = parts.iter().map(|g| GameDir::new(service, g)).collect();
        std::thread::Builder::new()
            .name("lanstash-delete".into())
            .spawn(move || shared.run(&dirs, files.as_ref()))
            .map(|_| ())
            .map_err(|e| {
                self.shared.finish();
                DeleteError::Spawn(e.to_string())
            })
    }

    /// The last deletion, running or finished.
    #[must_use]
    pub fn status(&self) -> Option<DeleteStatus> {
        self.shared.lock().clone()
    }

    /// Whether `platform`'s `game` is being deleted (any of it).
    #[must_use]
    pub fn deleting(&self, platform: &str, game: &str) -> bool {
        self.shared
            .lock()
            .as_ref()
            .is_some_and(|s| s.state == DeleteState::Running && s.platform == platform && s.game == game)
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<DeleteStatus>> {
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn update(&self, f: impl FnOnce(&mut DeleteStatus)) {
        if let Some(s) = self.lock().as_mut() {
            f(s);
        }
    }

    fn finish(&self) {
        let now = self.clock.now_ms();
        self.update(|s| {
            s.state = DeleteState::Done;
            s.finished_ms = Some(now);
        });
    }

    fn run(&self, dirs: &[GameDir], scope: Option<&Scope>) {
        let store = &self.store;
        let only = scope.map(|s| s.only.as_slice());
        let compact = scope.map_or(&[][..], |s| s.compact.as_slice());
        // The segment being written is not evictable: close it and let the records
        // in flight finish, so it seals (already sealed by the close: fine).
        for dir in dirs {
            if let (None, Some(active)) = (only, store.close_segment_of(dir)) {
                let _ = active.appender.seal_when_idle();
            }
        }
        let chosen = |v: &lanstash_store::evict::Victim| match only {
            None => true,
            Some(items) => items.iter().any(|(g, i)| *g == v.game && *i == v.item),
        };
        let victims: Vec<_> =
            store.eviction_candidates().into_iter().filter(|v| dirs.contains(&v.game) && chosen(v)).collect();
        let compacted = |v: &lanstash_store::evict::Victim| {
            compact.iter().any(|(g, seq, _)| *g == v.game && v.item == EvictItem::Segment(*seq))
        };
        let segments: u64 = store.eviction_candidates().iter().filter(|v| compacted(v)).map(|v| v.bytes).sum();
        let total: u64 = victims.iter().map(|v| v.bytes).sum::<u64>() + segments;
        self.update(|s| {
            s.items_total = (victims.len() + compact.len()) as u64;
            s.bytes_total = total;
        });
        let mut files = false;
        for v in &victims {
            match store.evict(v) {
                Ok(bytes) => {
                    files |= matches!(v.item, EvictItem::File(_));
                    self.events.emit(&Event::Evicted { path: &v.path, bytes, last_used_ms: v.last_used_ms });
                    self.update(|s| {
                        s.items_done += 1;
                        s.bytes_done += bytes;
                    });
                }
                Err(e) => {
                    let error = e.to_string();
                    self.events.emit(&Event::EvictFailed { path: &v.path, error: &error });
                    self.update(|s| {
                        s.items_done += 1;
                        s.failed.push((v.path.clone(), error));
                    });
                }
            }
        }
        for (dir, seq, keep) in compact {
            let path = dir.segment(*seq);
            match store.compact_segment(dir, *seq, &|k| keep.contains(k)) {
                Ok(c) => {
                    let (kept_bytes, freed_bytes) = (c.copied_bytes, c.freed_bytes());
                    self.events.emit(&Event::Compacted { path: &path, records: c.records, kept_bytes, freed_bytes });
                    self.update(|s| {
                        s.items_done += 1;
                        s.bytes_done += c.segment_bytes;
                        s.bytes_kept += kept_bytes;
                    });
                }
                Err(e) => {
                    let error = e.to_string();
                    self.events.emit(&Event::EvictFailed { path: &path, error: &error });
                    self.update(|s| {
                        s.items_done += 1;
                        s.failed.push((path.clone(), error));
                    });
                }
            }
        }
        if files {
            let failed = store.compact_journals().1;
            self.update(|s| s.failed.extend(failed.into_iter().map(|(g, e)| (g.journal(), e.to_string()))));
        }
        self.finish();
    }
}
