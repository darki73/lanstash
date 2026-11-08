//! The object-file journal (`journal.lsj`), one per game directory.
//!
//! An append-only list of entries; each declares an object file or a valid byte
//! range of one; a range's entry is appended only once its bytes are synced,
//! which makes it crash-safe.
//!
//! ```text
//! entry  = "LJ" | type u16 | payload_len u32 | payload | crc u32   (CRC-32C of type, payload_len, payload)
//! HEADER = version u16                                            (first entry of every journal)
//! OBJECT = no u32 | total u64 | key_len u16 key | path_len u16 path | meta_len u16 meta
//! EXTENT = no u32 | start u64 | len u64 | crc u32                 (CRC-32C of the bytes)
//! FORGET = no u32                                                 (the object file is gone or invalid)
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::checksum::crc::Crc32c;

pub const ENTRY_MAGIC: [u8; 2] = *b"LJ";
pub const JOURNAL_VERSION: u16 = 1;
/// Magic + type + length.
pub const ENTRY_HEAD_LEN: usize = 8;
/// Largest payload accepted (an OBJECT with 3 × 64 KiB strings).
pub const MAX_PAYLOAD: usize = 4 + 8 + 3 * (2 + 65_535);

const T_HEADER: u16 = 1;
const T_OBJECT: u16 = 2;
const T_EXTENT: u16 = 3;
const T_FORGET: u16 = 4;

/// One journal entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Header { version: u16 },
    Object { no: u32, total_len: u64, key: String, path: String, meta: Vec<u8> },
    Extent { no: u32, start: u64, len: u64, crc: u32 },
    Forget { no: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    /// A string or metadata field longer than 65535 bytes.
    FieldTooLong(&'static str),
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldTooLong(w) => write!(f, "journal entry field too long: {w}"),
        }
    }
}

impl core::error::Error for JournalError {}

fn put_field(out: &mut Vec<u8>, what: &'static str, b: &[u8]) -> Result<(), JournalError> {
    let n = u16::try_from(b.len()).map_err(|_| JournalError::FieldTooLong(what))?;
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(b);
    Ok(())
}

impl Entry {
    /// Encodes the entry with its framing and CRC.
    ///
    /// # Errors
    /// A key, path or metadata longer than 65535 bytes.
    pub fn encode(&self) -> Result<Vec<u8>, JournalError> {
        let mut p = Vec::new();
        let ty = match self {
            Self::Header { version } => {
                p.extend_from_slice(&version.to_le_bytes());
                T_HEADER
            }
            Self::Object { no, total_len, key, path, meta } => {
                p.extend_from_slice(&no.to_le_bytes());
                p.extend_from_slice(&total_len.to_le_bytes());
                put_field(&mut p, "key", key.as_bytes())?;
                put_field(&mut p, "path", path.as_bytes())?;
                put_field(&mut p, "meta", meta)?;
                T_OBJECT
            }
            Self::Extent { no, start, len, crc } => {
                p.extend_from_slice(&no.to_le_bytes());
                p.extend_from_slice(&start.to_le_bytes());
                p.extend_from_slice(&len.to_le_bytes());
                p.extend_from_slice(&crc.to_le_bytes());
                T_EXTENT
            }
            Self::Forget { no } => {
                p.extend_from_slice(&no.to_le_bytes());
                T_FORGET
            }
        };
        let len = u32::try_from(p.len()).map_err(|_| JournalError::FieldTooLong("payload"))?;
        let mut out = Vec::with_capacity(ENTRY_HEAD_LEN + p.len() + 4);
        out.extend_from_slice(&ENTRY_MAGIC);
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&p);
        let mut c = Crc32c::new();
        c.update(&out[2..]);
        out.extend_from_slice(&c.finish().to_le_bytes());
        Ok(out)
    }
}

/// Result of reading a journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRead {
    pub entries: Vec<Entry>,
    /// Bytes up to the end of the last valid entry; new entries go here.
    pub valid_len: u64,
    /// Why reading stopped before the end of the data, if it did.
    pub stopped: Option<String>,
    /// Bytes of damaged entries skipped in the middle (valid entries follow them).
    pub skipped: u64,
    /// The journal is not one this version reads (a newer version, or no valid header
    /// while valid entries are there): nothing is accepted, and it must be left as it is.
    pub refused: bool,
}

/// One whole entry at the start of `rest`: its type, payload and length, when its
/// framing and checksum hold.
fn entry_at(rest: &[u8]) -> Result<(u16, &[u8], usize), &'static str> {
    if rest.len() < ENTRY_HEAD_LEN {
        return Err("incomplete entry head");
    }
    if rest[..2] != ENTRY_MAGIC {
        return Err("bad entry magic");
    }
    let ty = u16::from_le_bytes([rest[2], rest[3]]);
    let len = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]) as usize;
    if len > MAX_PAYLOAD {
        return Err("entry too long");
    }
    let total = ENTRY_HEAD_LEN + len + 4;
    if rest.len() < total {
        return Err("incomplete entry");
    }
    let mut c = Crc32c::new();
    c.update(&rest[2..ENTRY_HEAD_LEN + len]);
    let stored = u32::from_le_bytes([rest[total - 4], rest[total - 3], rest[total - 2], rest[total - 1]]);
    if c.finish() != stored {
        return Err("entry checksum mismatch");
    }
    Ok((ty, &rest[ENTRY_HEAD_LEN..ENTRY_HEAD_LEN + len], total))
}

/// The offset of the next whole, valid entry after `from`, if there is one.
fn next_entry(data: &[u8], from: usize) -> Option<usize> {
    (from + 1..data.len().saturating_sub(1)).find(|&q| {
        data[q..].starts_with(&ENTRY_MAGIC)
            && entry_at(&data[q..]).is_ok_and(|(ty, payload, _)| decode_payload(ty, payload).is_ok())
    })
}

/// Little-endian field reader over one payload.
struct Fields<'a>(&'a [u8]);

impl<'a> Fields<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
    fn field(&mut self) -> Option<&'a [u8]> {
        let n = self.u16()?;
        self.take(usize::from(n))
    }
    fn string(&mut self) -> Option<String> {
        core::str::from_utf8(self.field()?).ok().map(String::from)
    }
}

fn decode_payload(ty: u16, payload: &[u8]) -> Result<Option<Entry>, &'static str> {
    let mut f = Fields(payload);
    let e = match ty {
        T_HEADER => Entry::Header { version: f.u16().ok_or("short header")? },
        T_OBJECT => Entry::Object {
            no: f.u32().ok_or("short object")?,
            total_len: f.u64().ok_or("short object")?,
            key: f.string().ok_or("bad object key")?,
            path: f.string().ok_or("bad object path")?,
            meta: f.field().ok_or("bad object meta")?.to_vec(),
        },
        T_EXTENT => Entry::Extent {
            no: f.u32().ok_or("short extent")?,
            start: f.u64().ok_or("short extent")?,
            len: f.u64().ok_or("short extent")?,
            crc: f.u32().ok_or("short extent")?,
        },
        T_FORGET => Entry::Forget { no: f.u32().ok_or("short forget")? },
        // Unknown types from a newer version are skipped, not fatal.
        _ => return Ok(None),
    };
    if !f.0.is_empty() {
        return Err("trailing bytes in entry");
    }
    Ok(Some(e))
}

/// Reads entries from the start of `data`. The first entry must be a
/// [`Entry::Header`] of this version. A damaged entry with valid ones after it is
/// skipped ([`JournalRead::skipped`]); one with nothing valid after it is a torn tail
/// (a write cut off by a crash), where reading stops. A journal of a newer version,
/// or one whose header is lost while valid entries follow, is
/// [`JournalRead::refused`]; one with no valid entry at all is empty (torn while
/// being created).
#[must_use]
pub fn read_journal(data: &[u8]) -> JournalRead {
    let mut entries = Vec::new();
    let mut pos = 0usize;
    let mut skipped = 0u64;
    let stop = |entries, pos: usize, skipped, why: &str| JournalRead {
        entries,
        valid_len: pos as u64,
        stopped: Some(String::from(why)),
        skipped,
        refused: false,
    };
    let refuse = |why: &str| JournalRead {
        entries: Vec::new(),
        valid_len: 0,
        stopped: Some(String::from(why)),
        skipped: 0,
        refused: true,
    };
    while pos < data.len() {
        let first = entries.is_empty();
        let read = entry_at(&data[pos..]).and_then(|(ty, payload, total)| Ok((decode_payload(ty, payload)?, total)));
        let (entry, total) = match read {
            Ok(r) => r,
            Err(why) => match next_entry(data, pos) {
                Some(_) if first => return refuse("no valid header, though valid entries follow"),
                Some(q) => {
                    skipped += (q - pos) as u64;
                    pos = q;
                    continue;
                }
                None if first => return stop(entries, 0, 0, why),
                None => return stop(entries, pos, skipped, why),
            },
        };
        match (&entry, first) {
            (Some(Entry::Header { version }), true) if *version == JOURNAL_VERSION => {}
            (Some(Entry::Header { version }), true) if *version > JOURNAL_VERSION => {
                return refuse("a journal of a newer version");
            }
            (_, true) => return refuse("journal does not start with a header of this version"),
            // Nothing a writer appends: skipped like a damaged entry.
            (Some(Entry::Header { .. }), false) => {
                skipped += total as u64;
                pos += total;
                continue;
            }
            _ => {}
        }
        if let Some(e) = entry {
            entries.push(e);
        }
        pos += total;
    }
    JournalRead { entries, valid_len: pos as u64, stopped: None, skipped, refused: false }
}

/// One object file as the journal describes it, with its extents in journal order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveObject {
    pub no: u32,
    pub total_len: u64,
    pub key: String,
    pub path: String,
    pub meta: Vec<u8>,
    /// `(start, len, crc)`: CRC-32C of the bytes `start..start + len`.
    pub extents: Vec<(u64, u64, u32)>,
}

/// The object files a journal still describes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiveJournal {
    pub objects: Vec<LiveObject>,
    /// Highest object number used plus one (forgotten ones included).
    pub next_no: u32,
    /// Entries that made no sense: a second object with the same number or
    /// key, an extent of an unknown object, an empty or out-of-range extent.
    pub bad: u64,
}

/// Folds journal entries into the object files they describe: forgotten
/// objects are dropped, nonsensical entries counted in [`LiveJournal::bad`].
#[must_use]
pub fn live_objects(entries: Vec<Entry>) -> LiveJournal {
    use alloc::collections::{BTreeMap, BTreeSet};
    let forgotten: BTreeSet<u32> =
        entries.iter().filter_map(|e| if let Entry::Forget { no } = e { Some(*no) } else { None }).collect();
    let mut out = LiveJournal::default();
    let mut by_no: BTreeMap<u32, usize> = BTreeMap::new();
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for e in entries {
        match e {
            Entry::Object { no, total_len, key, path, meta } => {
                out.next_no = out.next_no.max(no.saturating_add(1));
                if forgotten.contains(&no) {
                    continue;
                }
                if by_no.contains_key(&no) || keys.contains(&key) {
                    out.bad += 1;
                    continue;
                }
                by_no.insert(no, out.objects.len());
                keys.insert(key.clone());
                out.objects.push(LiveObject { no, total_len, key, path, meta, extents: Vec::new() });
            }
            Entry::Extent { no, start, len, crc } => {
                let Some(&i) = by_no.get(&no) else {
                    if !forgotten.contains(&no) {
                        out.bad += 1;
                    }
                    continue;
                };
                let obj = &mut out.objects[i];
                if len == 0 || start.checked_add(len).is_none_or(|end| end > obj.total_len) {
                    out.bad += 1;
                    continue;
                }
                obj.extents.push((start, len, crc));
            }
            Entry::Header { .. } | Entry::Forget { .. } => {}
        }
    }
    out
}
