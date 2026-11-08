//! Eviction support in the store: what could be
//! deleted, deleting it, and keeping the last-use times across restarts. The
//! policy (how much, which first) is in `lanstash_domain::evict`; the loop
//! that applies it is a use case in the app layer.

use std::sync::Arc;

use crate::layout::GameDir;
use crate::store::Store;
use lanstash_ports::storage::{StoreError, StoreFs};

/// One evictable thing of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvictItem {
    /// A segment by sequence number (small whole objects, up to its size).
    Segment(u32),
    /// An object file by journal number (one large object, its stored extents).
    File(u32),
}

/// Something eviction could delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Victim {
    pub game: GameDir,
    pub item: EvictItem,
    /// Root-relative path.
    pub path: String,
    pub bytes: u64,
    pub last_used_ms: u64,
    /// Index into [`Store::roots`]: the file system it is on.
    pub root: usize,
}

fn same(a: &Arc<dyn StoreFs>, b: &Arc<dyn StoreFs>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

impl Store {
    /// The distinct roots: the default one first, then per-service ones.
    #[must_use]
    pub fn roots(&self) -> Vec<Arc<dyn StoreFs>> {
        let r = self.roots_config();
        let mut out = vec![Arc::clone(&r.default)];
        let mut services: Vec<&String> = r.per_service.keys().collect();
        services.sort();
        for s in services {
            let fs = &r.per_service[s];
            if !out.iter().any(|o| same(o, fs)) {
                out.push(Arc::clone(fs));
            }
        }
        out
    }

    /// Everything that could be evicted (all but the segments being written).
    #[must_use]
    pub fn eviction_candidates(&self) -> Vec<Victim> {
        let roots = self.roots();
        let mut out = Vec::new();
        for game in self.all_games() {
            let root = roots.iter().position(|r| same(r, game.fs())).unwrap_or(0);
            for (item, path, bytes, last_used_ms) in game.candidates(self.index()) {
                out.push(Victim { game: game.dir.clone(), item, path, bytes, last_used_ms, root });
            }
        }
        out
    }

    /// A game's object files: (journal number, as in [`EvictItem::File`]; object key).
    #[must_use]
    pub fn object_files_of(&self, dir: &GameDir) -> Vec<(u32, String)> {
        self.existing_game(dir).map(|g| g.file_keys()).unwrap_or_default()
    }

    /// Deletes a victim. Returns the bytes it held.
    ///
    /// # Errors
    /// Read-only store, unknown game or item, or storage failure.
    pub fn evict(&self, v: &Victim) -> Result<u64, StoreError> {
        self.writable()?;
        let game = self.existing_game(&v.game).ok_or_else(|| StoreError::other("unknown game"))?;
        match v.item {
            EvictItem::Segment(seq) => game.evict_segment(self.index(), seq),
            EvictItem::File(no) => game.evict_file(self.index(), no),
        }
    }

    /// Writes every game's last-use times. Returns the games that failed.
    #[must_use]
    pub fn save_recency(&self) -> Vec<(GameDir, StoreError)> {
        if self.writable().is_err() {
            return Vec::new();
        }
        self.all_games()
            .into_iter()
            .filter_map(|g| g.save_recency(self.index()).err().map(|e| (g.dir.clone(), e)))
            .collect()
    }

    /// Bytes stored in the whole store (what `max_size` limits).
    #[must_use]
    pub fn stored_bytes(&self) -> u64 {
        self.index().stored_bytes()
    }
}

impl Store {
    /// Rewrites journals that are mostly dead entries (after object files were
    /// evicted). Returns the games compacted and the games that failed.
    #[must_use]
    pub fn compact_journals(&self) -> (Vec<GameDir>, Vec<(GameDir, StoreError)>) {
        let (mut done, mut failed) = (Vec::new(), Vec::new());
        if self.writable().is_err() {
            return (done, failed);
        }
        for g in self.all_games() {
            match g.compact_journal() {
                Ok(true) => done.push(g.dir.clone()),
                Ok(false) => {}
                Err(e) => failed.push((g.dir.clone(), e)),
            }
        }
        (done, failed)
    }
}
