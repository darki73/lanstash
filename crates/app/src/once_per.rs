//! A result worked out once per key: kept with the key it was worked out for, and worked
//! out again only when asked with another. For what is costly to work out and depends only
//! on things that say when they change (a stash's change counter, a library's read time):
//! asked twice in a row with nothing changed, it is worked out once.

use std::sync::{Arc, Mutex, PoisonError};

/// The last result and its key.
#[derive(Debug)]
pub struct OncePer<K, V> {
    kept: Mutex<Option<(K, Arc<V>)>>,
}

impl<K, V> Default for OncePer<K, V> {
    fn default() -> Self {
        Self { kept: Mutex::new(None) }
    }
}

impl<K: PartialEq, V> OncePer<K, V> {
    /// The result for `key`: the kept one when it was worked out for `key`, else `work`'s
    /// (kept for next time). `work` runs without the lock held, so a slow one blocks no reader
    /// of another key's result for long; two asking at once may both work it out.
    pub fn get(&self, key: K, work: impl FnOnce() -> V) -> Arc<V> {
        {
            let kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((k, v)) = kept.as_ref()
                && *k == key
            {
                return Arc::clone(v);
            }
        }
        let v = Arc::new(work());
        *self.kept.lock().unwrap_or_else(PoisonError::into_inner) = Some((key, Arc::clone(&v)));
        v
    }
}
