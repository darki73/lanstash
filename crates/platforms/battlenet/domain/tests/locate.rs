//! Tests for `lanstash_battlenet_domain::locate`.

use lanstash_battlenet_domain::locate::{Located, Place, Range, held_bytes, locate, ranges};
use lanstash_battlenet_domain::tact::IndexEntry;

fn key(b: u8) -> String {
    format!("{b:02x}").repeat(16)
}

fn at(b: u8, size: u64, offset: Option<u64>) -> IndexEntry {
    IndexEntry { key: key(b), size, offset }
}

#[test]
fn archives_first_then_the_file_index() {
    let wanted: Vec<(String, u64)> = [(1, 10), (2, 20), (3, 30), (4, 0), (1, 10)].map(|(b, s)| (key(b), s)).into();
    let archives = vec![vec![at(1, 10, Some(500))], vec![at(2, 20, Some(0)), at(1, 10, Some(9))]];
    let file_index = [at(3, 31, None)];
    let got = locate(&wanted, &archives, Some(&file_index));
    assert_eq!(
        got,
        vec![
            Located { key: key(1), place: Place::Archive { archive: 0, offset: 500, size: 10 } },
            Located { key: key(2), place: Place::Archive { archive: 1, offset: 0, size: 20 } },
            Located { key: key(3), place: Place::Loose { size: 31 } },
            Located { key: key(4), place: Place::Absent },
        ],
        "repeated keys once, the first archive wins"
    );
    // Without a file index, the rest is taken to be loose.
    assert_eq!(locate(&wanted, &archives, None)[3].place, Place::Loose { size: 0 });
    assert_eq!(got.iter().map(|l| l.place.size()).sum::<u64>(), 61);
}

#[test]
fn neighbouring_files_share_a_request() {
    let file = |archive, offset, size| Located { key: key(0), place: Place::Archive { archive, offset, size } };
    let files = [
        file(0, 100, 50),
        file(1, 0, 10),
        file(0, 0, 100), // adjacent to the first
        file(0, 160, 40),
        file(0, 1000, 1),
        file(0, 120, 10), // inside another
        Located { key: key(1), place: Place::Loose { size: 5 } },
        file(0, 2000, 0),
    ];
    assert_eq!(
        ranges(&files, 10),
        vec![
            Range { archive: 0, start: 0, end: 200 },
            Range { archive: 0, start: 1000, end: 1001 },
            Range { archive: 1, start: 0, end: 10 },
        ]
    );
    assert_eq!(ranges(&files, 0).len(), 4, "the 10-byte gap splits");
}

#[test]
fn bytes_of_the_files_already_stored() {
    use lanstash_domain::extent::Span;
    let s = |a, b| Span::new(a, b);
    let parts = [s(0, 10), s(10, 20), s(25, 30), s(40, 60), s(70, 80), s(95, 100)];
    // Stored: 0..15 and 15..30 touch (one run), 45..60 misses the start of 40..60, 70..100 overlapped twice.
    let stored = [s(0, 15), s(15, 30), s(45, 60), s(70, 90), s(85, 100)];
    assert_eq!(held_bytes(&parts, &stored), 10 + 10 + 5 + 10 + 5);
    assert_eq!(held_bytes(&parts, &[]), 0);
    assert_eq!(held_bytes(&[], &stored), 0);
    assert_eq!(held_bytes(&parts, &[s(0, 100)]), 60);
}
