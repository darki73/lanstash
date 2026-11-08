//! Tests for `lanstash_domain::journal`.

use lanstash_domain::checksum::crc::Crc32c;
use lanstash_domain::journal::*;

fn sample() -> Vec<Entry> {
    vec![
        Entry::Header { version: JOURNAL_VERSION },
        Entry::Object {
            no: 1,
            total_len: 795_075_223,
            key: "blizzard/tpr/hs/data/18/72/1872".into(),
            path: "obj/tpr/hs/data/18/72/1872".into(),
            meta: b"content-type=binary/octet-stream\n".to_vec(),
        },
        Entry::Extent { no: 1, start: 0, len: 1 << 24, crc: 0xDEAD_BEEF },
        Entry::Forget { no: 1 },
    ]
}

fn encode_all(es: &[Entry]) -> Vec<u8> {
    es.iter().flat_map(|e| e.encode().unwrap()).collect()
}

#[test]
fn round_trip() {
    let data = encode_all(&sample());
    let r = read_journal(&data);
    let whole =
        JournalRead { entries: sample(), valid_len: data.len() as u64, stopped: None, skipped: 0, refused: false };
    assert_eq!(r, whole);
    assert_eq!(read_journal(&[]).entries, vec![]);
}

#[test]
fn torn_tail_stops_at_last_valid_entry() {
    let data = encode_all(&sample());
    let head_and_object = encode_all(&sample()[..2]).len();
    for cut in head_and_object + 1..data.len() {
        let r = read_journal(&data[..cut]);
        assert!(r.entries.len() >= 2, "cut {cut}");
        assert!(r.valid_len as usize <= cut);
        assert!(r.stopped.is_some() || r.valid_len as usize == cut, "cut {cut}");
    }
}

#[test]
fn any_flipped_byte_is_detected() {
    let data = encode_all(&sample());
    for i in 0..data.len() {
        let mut bad = data.clone();
        bad[i] ^= 0x20;
        let r = read_journal(&bad);
        assert!(r.stopped.is_some() || r.skipped > 0 || r.refused, "flip at {i} not detected");
        assert!(r.entries.len() < 4);
    }
}

#[test]
fn a_damaged_entry_in_the_middle_loses_only_itself() {
    let data = encode_all(&sample());
    let header = encode_all(&sample()[..1]).len();
    let object = encode_all(&sample()[1..2]).len();
    for i in header..header + object {
        let mut bad = data.clone();
        bad[i] ^= 0x20;
        let r = read_journal(&bad);
        assert_eq!(r.entries, [&sample()[..1], &sample()[2..]].concat(), "flip at {i}");
        assert_eq!((r.valid_len, r.skipped, r.stopped), (data.len() as u64, object as u64, None), "flip at {i}");
    }
}

#[test]
fn needs_a_header_of_a_known_version() {
    let no_header = encode_all(&sample()[1..]);
    let r = read_journal(&no_header);
    assert!(r.entries.is_empty() && r.refused, "valid entries without a header: not to be started over");
    let mut v2 = Entry::Header { version: JOURNAL_VERSION + 1 }.encode().unwrap();
    v2.extend(encode_all(&sample()[1..]));
    let r = read_journal(&v2);
    assert!(r.entries.is_empty() && r.refused && r.valid_len == 0, "a newer journal is left alone");
    let mut twice = encode_all(&sample()[..1]);
    twice.extend(encode_all(&sample()[..1]));
    assert_eq!(read_journal(&twice).entries.len(), 1);
    let garbage = read_journal(b"garbage");
    assert!(garbage.entries.is_empty() && !garbage.refused && garbage.valid_len == 0, "nothing to keep");
}

#[test]
fn unknown_entry_types_are_skipped() {
    let mut data = encode_all(&sample()[..1]);
    let mut unknown = vec![b'L', b'J', 99, 0, 1, 0, 0, 0, 7];
    let mut c = Crc32c::new();
    c.update(&unknown[2..]);
    unknown.extend_from_slice(&c.finish().to_le_bytes());
    data.extend(unknown);
    data.extend(encode_all(&sample()[2..3]));
    let r = read_journal(&data);
    assert_eq!(r.entries.len(), 2);
    assert_eq!(r.stopped, None);
}

#[test]
fn rejects_overlong_fields() {
    let e = Entry::Object { no: 1, total_len: 1, key: "k".repeat(70_000), path: "p".into(), meta: vec![] };
    assert_eq!(e.encode(), Err(JournalError::FieldTooLong("key")));
}

fn object(no: u32, key: &str, total_len: u64) -> Entry {
    Entry::Object { no, total_len, key: key.into(), path: format!("obj/{key}"), meta: Vec::new() }
}

#[test]
fn live_objects_fold_forgets_and_counts_nonsense() {
    let entries = vec![
        Entry::Header { version: JOURNAL_VERSION },
        object(0, "a", 100),
        Entry::Extent { no: 0, start: 0, len: 10, crc: 1 },
        object(1, "b", 50),
        Entry::Extent { no: 1, start: 0, len: 50, crc: 2 },
        Entry::Forget { no: 1 },
        Entry::Extent { no: 0, start: 90, len: 20, crc: 3 }, // past the end
        Entry::Extent { no: 0, start: 5, len: 0, crc: 4 },   // empty
        Entry::Extent { no: 7, start: 0, len: 1, crc: 5 },   // unknown object
        object(2, "a", 100),                                 // key used twice
        object(0, "c", 10),                                  // number used twice
        Entry::Extent { no: 0, start: 10, len: 90, crc: 6 },
    ];
    let live = live_objects(entries);
    assert_eq!(live.bad, 5);
    assert_eq!(live.next_no, 3);
    assert_eq!(live.objects.len(), 1);
    let a = &live.objects[0];
    assert_eq!((a.no, a.key.as_str(), a.total_len), (0, "a", 100));
    assert_eq!(a.extents, vec![(0, 10, 1), (10, 90, 6)]);
}
