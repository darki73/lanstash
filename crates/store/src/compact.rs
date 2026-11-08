//! Compacting a segment: a sealed segment that holds
//! some objects still wanted among many that are not has its wanted objects
//! copied into the game's active segment, then goes as a whole. Which objects
//! are wanted is the caller's (a platform's) decision.
//!
//! The copies are written and synced first and served instead of the originals in
//! one index step, so no lookup finds them missing; then the old file is deleted.
//! After a crash in between, both copies are on disk: the next start serves the
//! older one, and the newer is dead weight a later compaction drops.

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::record::RecordHeader;
use lanstash_ports::storage::StoreError;

use crate::layout::GameDir;
use crate::store::{ObjectRef, Store};

/// What a compaction did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Compacted {
    /// Objects copied, and their bytes.
    pub records: u64,
    pub copied_bytes: u64,
    /// Size of the segment deleted.
    pub segment_bytes: u64,
}

impl Compacted {
    /// Space given back: the segment, less what was copied out of it.
    #[must_use]
    pub fn freed_bytes(&self) -> u64 {
        self.segment_bytes.saturating_sub(self.copied_bytes)
    }
}

impl Store {
    /// Copies the objects of sealed segment `seq` of `dir` whose key `keep`
    /// accepts into the game's active segment, then deletes the segment. An
    /// object served from another copy, or whose body fails its checksum, is
    /// not copied (it is fetched again when asked for).
    ///
    /// # Errors
    /// Read-only store, unknown game, unknown or active segment, or storage
    /// failure (the segment then stays, and copies already written are dead
    /// weight a later compaction drops).
    pub fn compact_segment(
        &self,
        dir: &GameDir,
        seq: u32,
        keep: &dyn Fn(&str) -> bool,
    ) -> Result<Compacted, StoreError> {
        self.writable()?;
        let game = self.existing_game(dir).ok_or_else(|| StoreError::other("unknown game"))?;
        let (id, _) = game.sealed_segment(seq)?;
        let reader = self.index().segment(id).ok_or_else(|| StoreError::other("no such segment"))?.reader;
        let mut done = Compacted::default();
        let mut copies = Vec::new();
        for r in self.index().records_in(id) {
            if !self.index().serves(&r) {
                continue;
            }
            let mut head = vec![0u8; r.header_len as usize];
            reader.read_exact_at(r.offset, &mut head)?;
            let Ok(h) = RecordHeader::decode(&head) else { continue };
            if h.body_len != r.body_len || !keep(&h.key) {
                continue;
            }
            let len = usize::try_from(r.body_len).map_err(|_| StoreError::other("record too large"))?;
            let mut body = vec![0u8; len];
            reader.read_exact_at(r.offset + u64::from(r.header_len), &mut body)?;
            if crc32c(&body) != h.body_crc {
                continue;
            }
            let obj = ObjectRef { service: &dir.service, game: &dir.game, key: &h.key };
            let w = self.reserve_record(obj, &h.meta, r.body_len)?;
            w.write(0, &body)?;
            copies.push(w.commit_at(h.flags, h.body_crc)?);
            done.records += 1;
            done.copied_bytes += r.body_len;
        }
        done.segment_bytes = game.replace_segment(self.index(), seq, &copies)?;
        Ok(done)
    }
}
