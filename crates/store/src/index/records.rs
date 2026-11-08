//! A segment's records one by one: listing them, whether each is the copy an
//! object is served from, and moving objects to copies (compaction).

use lanstash_domain::extent::Span;

use super::{Index, Loc, SegId, SegInfo};

/// One whole-object record: the object's key hash and where the record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordLoc {
    pub hash: u64,
    pub seg: SegId,
    pub offset: u64,
    pub header_len: u32,
    pub body_len: u64,
}

impl Index {
    /// The records of a segment, by offset.
    #[must_use]
    pub fn records_in(&self, seg: SegId) -> Vec<RecordLoc> {
        self.read().by_offset.get(seg as usize).map_or_else(Vec::new, |list| {
            list.iter()
                .map(|r| RecordLoc {
                    hash: r.hash,
                    seg,
                    offset: r.offset,
                    header_len: r.header_len,
                    body_len: r.body_len,
                })
                .collect()
        })
    }

    /// Whether the object of `r.hash` is served from that record (and not from
    /// another copy of it).
    #[must_use]
    pub fn serves(&self, r: &RecordLoc) -> bool {
        let i = self.read();
        i.objs.get(&r.hash).is_some_and(|o| {
            !o.unusable
                && o.pieces
                    .iter()
                    .any(|(_, l)| matches!(l, Loc::Record { seg, offset, .. } if *seg == r.seg && *offset == r.offset))
        })
    }

    /// Forgets segment `old` and, in the same step, serves the objects copied
    /// out of it from their copies (records already written, see
    /// [`crate::store::RecordWriter::commit_at`]). No lookup sees them missing in
    /// between. Returns the old segment's info, if it was still registered.
    pub fn replace_segment(&self, old: SegId, copies: &[RecordLoc]) -> Option<SegInfo> {
        let mut i = self.write();
        let info = i.segs.get_mut(old as usize).and_then(Option::take);
        if let Some(s) = &info {
            // What its service holds changed (evicted, deleted, pruned, compacted).
            i.changed(&s.game.service);
            let records = std::mem::take(&mut i.by_offset[old as usize]);
            for r in records {
                i.drop_pieces(r.hash, |l| matches!(l, Loc::Record { seg: s, .. } if *s == old), None);
            }
        }
        for c in copies {
            if i.seg(c.seg).is_some() {
                let loc = Loc::Record { seg: c.seg, offset: c.offset, header_len: c.header_len };
                i.insert(c.hash, c.body_len, Span::at(0, c.body_len), loc);
            }
        }
        info
    }
}
