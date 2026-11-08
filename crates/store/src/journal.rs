//! A game's journal file (`journal.lsj`): reading it back at startup and
//! appending entries. The format is in [`lanstash_domain::journal`].

use std::sync::Arc;

use lanstash_domain::journal::{Entry, JOURNAL_VERSION, JournalRead, read_journal};

use lanstash_ports::storage::{FileWrite, StoreError, StoreErrorKind, StoreFs};

/// An open journal, positioned for appending.
pub struct JournalFile {
    file: Arc<dyn FileWrite>,
    len: u64,
}

impl std::fmt::Debug for JournalFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JournalFile").field("len", &self.len).finish_non_exhaustive()
    }
}

/// What opening an existing journal found.
#[derive(Debug, Clone)]
pub struct JournalOpened {
    pub read: JournalRead,
    /// Bytes cut off after the last valid entry (a write interrupted by a crash).
    pub truncated: u64,
}

impl JournalFile {
    /// Creates a new journal with its header entry.
    ///
    /// # Errors
    /// The file exists or can't be written.
    pub fn create(fs: &dyn StoreFs, path: &str) -> Result<Self, StoreError> {
        let file = fs.create_new(path)?;
        let mut j = Self { file, len: 0 };
        j.append(&Entry::Header { version: JOURNAL_VERSION })?;
        Ok(j)
    }

    /// Opens an existing journal, reads all valid entries and cuts off a torn tail
    /// so new entries follow the last valid one. A journal with no valid entry at all
    /// (torn while being created) is started again.
    ///
    /// # Errors
    /// The file can't be read or written, or it is not a journal this version reads
    /// ([`JournalRead::refused`]): then it is left untouched.
    pub fn open(fs: &dyn StoreFs, path: &str) -> Result<(Self, JournalOpened), StoreError> {
        let file = fs.open_write(path)?;
        let size = file.size()?;
        let mut data = vec![0u8; usize::try_from(size).map_err(|_| StoreError::other("journal too large"))?];
        file.read_exact_at(0, &mut data)?;
        let read = read_journal(&data);
        if read.refused {
            let why = read.stopped.unwrap_or_default();
            return Err(StoreError::other(format!("{path}: {why}; left as it is")));
        }
        let truncated = size - read.valid_len;
        if truncated > 0 {
            file.set_len(read.valid_len)?;
        }
        let mut j = Self { file, len: read.valid_len };
        if read.valid_len == 0 {
            j.append(&Entry::Header { version: JOURNAL_VERSION })?;
        }
        Ok((j, JournalOpened { read, truncated }))
    }

    /// Opens the journal at `path`, or creates it if it does not exist.
    ///
    /// # Errors
    /// Storage failure.
    pub fn open_or_create(fs: &dyn StoreFs, path: &str) -> Result<(Self, Option<JournalOpened>), StoreError> {
        match Self::open(fs, path) {
            Ok((j, o)) => Ok((j, Some(o))),
            Err(e) if e.kind == StoreErrorKind::NotFound => Ok((Self::create(fs, path)?, None)),
            Err(e) => Err(e),
        }
    }

    /// Appends one entry (not synced: a lost entry only means a download is repeated).
    ///
    /// # Errors
    /// Encoding or storage failure. On a storage failure the journal is cut back,
    /// so a partly written entry can't hide later ones.
    pub fn append(&mut self, e: &Entry) -> Result<(), StoreError> {
        let bytes = e.encode().map_err(|err| StoreError::other(err.to_string()))?;
        if let Err(err) = self.file.write_all_at(self.len, &bytes) {
            let _ = self.file.set_len(self.len);
            return Err(err);
        }
        self.len += bytes.len() as u64;
        Ok(())
    }

    /// Makes the entries appended so far durable.
    ///
    /// # Errors
    /// Storage failure.
    pub fn sync(&self) -> Result<(), StoreError> {
        self.file.sync()
    }

    #[must_use]
    pub fn len(&self) -> u64 {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
