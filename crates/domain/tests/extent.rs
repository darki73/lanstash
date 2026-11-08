//! Tests for `lanstash_domain::extent`.

use lanstash_domain::extent::*;

/// A value that joins with the same file id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct F(u8);
impl Joinable for F {
    fn joins(&self, next: &Self) -> bool {
        self == next
    }
}

fn spans(m: &ExtentMap<F>) -> Vec<(u64, u64, u8)> {
    m.iter().map(|(s, v)| (s.start, s.end, v.0)).collect()
}

#[test]
fn span_basics() {
    let s = Span::new(10, 20);
    assert_eq!((s.len(), s.is_empty(), s.contains(10), s.contains(20)), (10, false, true, false));
    assert!(Span::new(5, 5).is_empty());
    assert_eq!(s.intersect(&Span::new(15, 30)), Some(Span::new(15, 20)));
    assert_eq!(s.intersect(&Span::new(20, 30)), None);
    assert_eq!(Span::at(u64::MAX - 1, 10), Span::new(u64::MAX - 1, u64::MAX));
}

#[test]
fn inserts_in_any_order_and_joins_neighbours() {
    let mut m = ExtentMap::new();
    m.insert(Span::new(20, 30), F(1)).unwrap();
    m.insert(Span::new(0, 10), F(1)).unwrap();
    m.insert(Span::new(40, 50), F(2)).unwrap();
    assert_eq!(spans(&m), vec![(0, 10, 1), (20, 30, 1), (40, 50, 2)]);
    m.insert(Span::new(10, 20), F(1)).unwrap(); // fills the hole: joins both sides
    assert_eq!(spans(&m), vec![(0, 30, 1), (40, 50, 2)]);
    m.insert(Span::new(30, 40), F(1)).unwrap(); // joins left only (right has another value)
    assert_eq!(spans(&m), vec![(0, 40, 1), (40, 50, 2)]);
    m.insert(Span::new(55, 60), F(2)).unwrap();
    m.insert(Span::new(50, 55), F(2)).unwrap();
    assert_eq!(spans(&m), vec![(0, 40, 1), (40, 60, 2)]);
    assert_eq!(m.covered_bytes(), 60);
    m.insert(Span::new(70, 70), F(3)).unwrap(); // empty: ignored
    assert_eq!(m.len(), 2);
}

#[test]
fn rejects_overlaps_without_changes() {
    let mut m = ExtentMap::new();
    m.insert(Span::new(10, 20), F(1)).unwrap();
    for bad in [Span::new(5, 11), Span::new(19, 25), Span::new(12, 13), Span::new(0, 100)] {
        assert_eq!(m.insert(bad, F(1)), Err(Overlap { existing: Span::new(10, 20), new: bad }), "{bad:?}");
    }
    assert_eq!(spans(&m), vec![(10, 20, 1)]);
    m.insert(Span::new(20, 21), F(9)).unwrap(); // touching is fine
    m.insert(Span::new(9, 10), F(9)).unwrap();
    assert_eq!(spans(&m), vec![(9, 10, 9), (10, 20, 1), (20, 21, 9)]);
}

#[test]
fn lookups_and_gaps() {
    let mut m = ExtentMap::new();
    m.insert(Span::new(10, 20), F(1)).unwrap();
    m.insert(Span::new(30, 40), F(2)).unwrap();
    assert_eq!(m.at(9), None);
    assert_eq!(m.at(10).map(|(s, v)| (s, v.0)), Some((Span::new(10, 20), 1)));
    assert_eq!(m.at(19).map(|(s, _)| s.start), Some(10));
    assert_eq!(m.at(20), None);
    assert_eq!(m.at(35).map(|(_, v)| v.0), Some(2));
    assert_eq!(m.next_start_after(0), Some(10));
    assert_eq!(m.next_start_after(10), Some(30));
    assert_eq!(m.next_start_after(25), Some(30));
    assert_eq!(m.next_start_after(30), None);
    assert_eq!(m.gaps(Span::new(0, 50)), vec![Span::new(0, 10), Span::new(20, 30), Span::new(40, 50)]);
    assert_eq!(m.gaps(Span::new(12, 35)), vec![Span::new(20, 30)]);
    assert_eq!(m.gaps(Span::new(12, 18)), vec![]);
    assert_eq!(m.gaps(Span::new(22, 28)), vec![Span::new(22, 28)]);
    assert!(m.covers(Span::new(30, 40)) && !m.covers(Span::new(30, 41)) && m.covers(Span::new(5, 5)));
}

#[test]
fn gaps_and_inserts_agree_on_random_input() {
    // Deterministic pseudo-random inserts; gaps must exactly complement the map.
    let mut m = ExtentMap::new();
    let mut covered = [false; 512];
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..400 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let a = x % 500;
        let len = 1 + (x >> 20) % 12;
        let span = Span::new(a, (a + len).min(512));
        let free = (span.start..span.end).all(|i| !covered[i as usize]);
        let r = m.insert(span, F((x >> 40) as u8 % 3));
        assert_eq!(r.is_ok(), free, "{span:?}");
        if free {
            for i in span.start..span.end {
                covered[i as usize] = true;
            }
        }
    }
    let gaps = m.gaps(Span::new(0, 512));
    let mut from_gaps = [true; 512];
    for g in &gaps {
        for i in g.start..g.end {
            from_gaps[i as usize] = false;
        }
    }
    assert_eq!(from_gaps, covered);
    for (i, &c) in covered.iter().enumerate() {
        assert_eq!(m.at(i as u64).is_some(), c, "at({i})");
    }
    // Stored extents are ordered and never touch with joinable values.
    let items: Vec<_> = m.iter().collect();
    for w in items.windows(2) {
        assert!(w[0].0.end <= w[1].0.start);
        assert!(!(w[0].0.end == w[1].0.start && w[0].1 == w[1].1), "unjoined neighbours {w:?}");
    }
}

#[test]
fn uncovered_skips_taken_spans() {
    let t = [Span::new(10, 20), Span::new(15, 25), Span::new(40, 50)];
    assert_eq!(uncovered(Span::new(0, 60), &t), vec![Span::new(0, 10), Span::new(25, 40), Span::new(50, 60)]);
    assert_eq!(uncovered(Span::new(12, 45), &t), vec![Span::new(25, 40)]);
    assert_eq!(uncovered(Span::new(12, 18), &t), vec![]);
    assert_eq!(uncovered(Span::new(0, 5), &[]), vec![Span::new(0, 5)]);
}
