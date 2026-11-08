//! The store: one or more roots (a default one, optionally one per service),
//! the games in them, and the index over everything.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lanstash_domain::extent::Span;
use lanstash_domain::segment::footer::key_hash;
use lanstash_domain::segment::record::RecordFlags;

use crate::game::{ActiveSegment, Game, GameReport, ObjectFile, SegmentOptions};
use crate::index::records::RecordLoc;
use crate::index::{Index, Lookup, Usage};
use crate::layout::GameDir;
use crate::segment::writer::Reservation;
use lanstash_ports::clock::Clock;
use lanstash_ports::storage::{FileRead, StoreError, StoreErrorKind, StoreFs};

/// Store roots: a default, and optional per-service overrides.
#[derive(Clone)]
pub struct Roots {
    pub default: Arc<dyn StoreFs>,
    pub per_service: HashMap<String, Arc<dyn StoreFs>>,
}

impl std::fmt::Debug for Roots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s: Vec<&String> = self.per_service.keys().collect();
        s.sort();
        f.debug_struct("Roots").field("per_service", &s).finish_non_exhaustive()
    }
}

impl Roots {
    #[must_use]
    pub fn single(fs: Arc<dyn StoreFs>) -> Self {
        Self { default: fs, per_service: HashMap::new() }
    }

    #[must_use]
    pub fn for_service(&self, service: &str) -> &Arc<dyn StoreFs> {
        self.per_service.get(service).unwrap_or(&self.default)
    }

    /// Every game directory, each in the root responsible for its service.
    ///
    /// # Errors
    /// A root exists but can't be listed.
    pub fn games(&self) -> Result<FoundGames, StoreError> {
        let mut out = FoundGames::default();
        // Each root once, with the services it is responsible for.
        let mut visits: Vec<(Arc<dyn StoreFs>, Option<String>)> = vec![(Arc::clone(&self.default), None)];
        let mut services: Vec<&String> = self.per_service.keys().collect();
        services.sort();
        for s in services {
            visits.push((Arc::clone(&self.per_service[s]), Some(s.clone())));
        }
        let mut seen = HashSet::new();
        for (fs, only) in visits {
            let entries = match fs.list("") {
                Ok(e) => e,
                Err(e) if e.kind == StoreErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            for svc in entries.into_iter().filter(|e| e.is_dir) {
                let mine = match &only {
                    Some(s) => *s == svc.name,
                    None => !self.per_service.contains_key(&svc.name) || Arc::ptr_eq(&self.per_service[&svc.name], &fs),
                };
                if !mine {
                    out.ignored.push(svc.name.clone());
                    continue;
                }
                let gs = match fs.list(&svc.name) {
                    Ok(g) => g,
                    Err(e) => {
                        out.failed.push((svc.name.clone(), e.to_string()));
                        continue;
                    }
                };
                for g in gs.into_iter().filter(|e| e.is_dir) {
                    let dir = GameDir::from_listing(&svc.name, &g.name);
                    if seen.insert(dir.clone()) {
                        out.games.push((Arc::clone(&fs), dir));
                    }
                }
            }
        }
        Ok(out)
    }
}

/// What [`Roots::games`] found.
#[derive(Default)]
pub struct FoundGames {
    pub games: Vec<(Arc<dyn StoreFs>, GameDir)>,
    /// Service directories in a root that belong to another root.
    pub ignored: Vec<String>,
    /// Service directories that could not be listed: (path, error).
    pub failed: Vec<(String, String)>,
}

impl std::fmt::Debug for FoundGames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let games: Vec<&GameDir> = self.games.iter().map(|(_, g)| g).collect();
        f.debug_struct("FoundGames").field("games", &games).field("ignored", &self.ignored).finish_non_exhaustive()
    }
}

/// Store settings (passed in from the composition root).
#[derive(Debug, Clone, Copy, Default)]
pub struct StoreOptions {
    pub segments: SegmentOptions,
    /// Load without writing anything (no segment is continued or sealed, no
    /// journal is repaired) and refuse writes: safe next to a running server.
    pub read_only: bool,
}

/// What opening the store found.
#[derive(Debug, Clone, Default)]
pub struct LoadReport {
    pub games: u64,
    pub totals: GameReport,
    /// Service directories in a root that belong to another root (not loaded).
    pub ignored: Vec<String>,
}

/// Where a game is, for one object.
#[derive(Debug, Clone, Copy)]
pub struct ObjectRef<'a> {
    pub service: &'a str,
    pub game: &'a str,
    pub key: &'a str,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The store. Shared with `Arc`.
pub struct Store {
    roots: Roots,
    index: Arc<Index>,
    games: Mutex<HashMap<GameDir, Arc<Game>>>,
    opts: StoreOptions,
    clock: Arc<dyn Clock>,
    segment_ids: Arc<AtomicU64>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("index", &self.index).field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl Store {
    /// Opens all roots and loads every game into the index.
    ///
    /// # Errors
    /// A root can't be listed.
    pub fn open(roots: Roots, opts: StoreOptions, clock: Arc<dyn Clock>) -> Result<(Self, LoadReport), StoreError> {
        let segment_ids = Arc::new(AtomicU64::new(clock.now_ms().saturating_mul(1000)));
        let index = Arc::new(Index::new());
        let mut games = HashMap::new();
        let found = roots.games()?;
        let mut report = LoadReport { ignored: found.ignored, ..LoadReport::default() };
        report.totals.failed = found.failed;
        for (fs, dir) in found.games {
            let (game, r) =
                Game::load(dir.clone(), fs, Arc::clone(&segment_ids), &index, opts.read_only, clock.now_ms());
            report.games += 1;
            report.totals.add(&r);
            games.insert(dir, Arc::new(game));
        }
        let store = Self { roots, index, games: Mutex::new(games), opts, clock, segment_ids };
        Ok((store, report))
    }

    pub(crate) fn writable(&self) -> Result<(), StoreError> {
        if self.opts.read_only {
            return Err(StoreError::other("the store was opened read-only"));
        }
        Ok(())
    }

    #[must_use]
    pub fn index(&self) -> &Arc<Index> {
        &self.index
    }

    pub(crate) fn roots_config(&self) -> &Roots {
        &self.roots
    }

    pub(crate) fn all_games(&self) -> Vec<Arc<Game>> {
        let mut g: Vec<Arc<Game>> = lock(&self.games).values().cloned().collect();
        g.sort_by(|a, b| a.dir.cmp(&b.dir));
        g
    }

    pub(crate) fn existing_game(&self, dir: &GameDir) -> Option<Arc<Game>> {
        lock(&self.games).get(dir).cloned()
    }

    /// The object of `key`, marked as used now (a client asked for it).
    #[must_use]
    pub fn lookup(&self, key: &str) -> Lookup {
        self.index.lookup_touch(key, self.clock.now_ms())
    }

    /// The object of `key`, not marked as used (lanstash's own reads).
    #[must_use]
    pub fn lookup_quietly(&self, key: &str) -> Lookup {
        self.index.lookup(key)
    }

    /// The game directory for (service, game), created in memory if new.
    #[must_use]
    pub fn game(&self, service: &str, game: &str) -> Arc<Game> {
        let dir = GameDir::new(service, game);
        let mut g = lock(&self.games);
        Arc::clone(g.entry(dir.clone()).or_insert_with(|| {
            Arc::new(Game::empty(dir, Arc::clone(self.roots.for_service(service)), Arc::clone(&self.segment_ids)))
        }))
    }

    /// Reserves a segment record for a whole object of `body_len` bytes.
    ///
    /// # Errors
    /// Storage failure.
    pub fn reserve_record(&self, obj: ObjectRef<'_>, meta: &[u8], body_len: u64) -> Result<RecordWriter, StoreError> {
        self.writable()?;
        let game = self.game(obj.service, obj.game);
        let (seg, res) = game.reserve_record(&self.index, self.opts.segments, &*self.clock, obj.key, meta, body_len)?;
        self.index.touch_segment(seg.id, self.clock.now_ms());
        Ok(RecordWriter { index: Arc::clone(&self.index), seg, res: Some(res), hash: key_hash(obj.key.as_bytes()) })
    }

    /// The object file for `obj`, created at `total` bytes if new.
    ///
    /// # Errors
    /// A conflicting total, or storage failure.
    pub fn object_file(
        &self,
        obj: ObjectRef<'_>,
        cdn_path: &str,
        total: u64,
        meta: &[u8],
    ) -> Result<Arc<ObjectFile>, StoreError> {
        self.writable()?;
        if let Some(f) = self.index.file_of(obj.key) {
            return if f.total == total {
                self.index.touch_file(f.id(), self.clock.now_ms());
                Ok(f)
            } else {
                Err(StoreError::other(format!("{}: stored total {} differs from {total}", obj.key, f.total)))
            };
        }
        let f = self.game(obj.service, obj.game).object_file(&self.index, obj.key, cdn_path, total, meta)?;
        self.index.touch_file(f.id(), self.clock.now_ms());
        Ok(f)
    }

    /// Makes `span` of `file` durable and valid (sync, journal, index).
    ///
    /// # Errors
    /// Storage failure.
    pub fn commit_extent(&self, file: &ObjectFile, span: Span, crc: u32) -> Result<(), StoreError> {
        self.writable()?;
        let game = lock(&self.games).get(&file.game).cloned().ok_or_else(|| StoreError::other("unknown game"))?;
        game.commit_extent(&self.index, file, span, crc)?;
        self.index.touch_file(file.id(), self.clock.now_ms());
        Ok(())
    }

    /// Closes every active segment; each seals as soon as its open reservations end.
    pub fn close_segments(&self) -> Vec<Arc<ActiveSegment>> {
        let games: Vec<Arc<Game>> = lock(&self.games).values().cloned().collect();
        games.iter().filter_map(|g| g.close_active()).collect()
    }

    /// Closes one game's active segment, if it has one (it seals once the
    /// records being written into it are done; the game's next record opens a new one).
    pub fn close_segment_of(&self, dir: &GameDir) -> Option<Arc<ActiveSegment>> {
        self.existing_game(dir)?.close_active()
    }

    /// Stored objects and bytes per game.
    #[must_use]
    pub fn usage(&self) -> BTreeMap<GameDir, Usage> {
        self.index.usage()
    }

    /// How many times what `service` holds changed since the stash was opened (a cheap
    /// look: telling whether anything of it changed costs no walk over the index).
    #[must_use]
    pub fn changes(&self, service: &str) -> u64 {
        self.index.changes(service)
    }

    /// How many times what any service holds changed since the stash was opened.
    #[must_use]
    pub fn all_changes(&self) -> u64 {
        self.index.all_changes()
    }

    /// Whether the stash holds anything of `service`'s `game` (a deleted game holds nothing).
    #[must_use]
    pub fn holds(&self, service: &str, game: &str) -> bool {
        self.index.holds(&GameDir::new(service, game))
    }
}

/// A reserved segment record being written. Dropping it without
/// [`RecordWriter::commit`] abandons the reservation.
pub struct RecordWriter {
    index: Arc<Index>,
    seg: Arc<ActiveSegment>,
    res: Option<Reservation>,
    hash: u64,
}

impl std::fmt::Debug for RecordWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordWriter").field("segment", &self.seg.path).finish_non_exhaustive()
    }
}

impl RecordWriter {
    /// Writes body bytes at `at` (relative to the body start).
    ///
    /// # Errors
    /// Storage failure, or a write past the reserved body.
    pub fn write(&self, at: u64, data: &[u8]) -> Result<(), StoreError> {
        let r = self.res.as_ref().ok_or_else(|| StoreError::other("reservation already ended"))?;
        self.seg.appender.write_body(r, at, data)
    }

    /// A reader and the file offset of body byte 0, to read back written bytes.
    #[must_use]
    pub fn read_back(&self) -> (Arc<dyn FileRead>, u64) {
        let reader: Arc<dyn FileRead> = Arc::clone(self.seg.appender.file()) as Arc<dyn FileRead>;
        (reader, self.res.as_ref().map_or(0, Reservation::body_offset))
    }

    /// Commits the record (`body_crc` = CRC-32C of the whole body) and indexes it.
    ///
    /// # Errors
    /// Storage failure; the record is then not stored.
    pub fn commit(self, flags: RecordFlags, body_crc: u32) -> Result<(), StoreError> {
        self.commit_at(flags, body_crc).map(|_| ())
    }

    /// [`RecordWriter::commit`], returning where the record went. A record of
    /// an object already stored elsewhere is written but not served (the index
    /// keeps the first copy), until [`Index::replace_segment`] moves it there.
    ///
    /// # Errors
    /// Storage failure; the record is then not stored.
    pub fn commit_at(mut self, flags: RecordFlags, body_crc: u32) -> Result<RecordLoc, StoreError> {
        let r = self.res.take().ok_or_else(|| StoreError::other("reservation already ended"))?;
        let loc = self.seg.appender.commit(r, flags, body_crc).map_err(|e| StoreError::other(e.to_string()))?;
        self.index.insert_record(self.hash, self.seg.id, loc.offset, loc.header_len, loc.body_len);
        Ok(RecordLoc {
            hash: self.hash,
            seg: self.seg.id,
            offset: loc.offset,
            header_len: loc.header_len,
            body_len: loc.body_len,
        })
    }
}

impl Drop for RecordWriter {
    fn drop(&mut self) {
        if let Some(r) = self.res.take() {
            self.seg.appender.abandon(r);
        }
    }
}
