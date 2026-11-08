//! How many segments, object files and objects the index holds.

use super::Index;

/// What the index holds now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub segments: u64,
    pub object_files: u64,
    pub objects: u64,
}

impl Index {
    /// Segments registered so far (evicted ones included).
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.read().segs.len()
    }

    /// The segments and object files held now, and the objects.
    #[must_use]
    pub fn counts(&self) -> Counts {
        let i = self.read();
        Counts {
            segments: i.segs.iter().filter(|s| s.is_some()).count() as u64,
            object_files: i.files.iter().filter(|f| f.is_some()).count() as u64,
            objects: i.objs.len() as u64,
        }
    }
}
