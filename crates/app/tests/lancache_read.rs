//! Reading byte ranges of CDN files out of an nginx (lancache) cache by slice key.

use std::sync::Arc;

use lanstash_app::lancache_read::{LancacheOptions, LancacheRead, Next};
use lanstash_domain::nginx_cache::slice_key;
use lanstash_store::test_support::nginx::{MemNginx, cache_file};

const SLICE: u64 = 16;
const T: &str = "/tpr/ovw/data/ab/cd/abcd";
const BASE: &str = "blizzard/tpr/ovw/data/ab/cd/abcd";

fn body(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 % 251) as u8).collect()
}

fn reader(n: &Arc<MemNginx>) -> LancacheRead {
    let opts = LancacheOptions { service: "blizzard".into(), slice_bytes: SLICE, max_piece: 1000 };
    LancacheRead::new(Arc::clone(n) as _, opts)
}

#[test]
fn whole_files_and_ranges_come_from_their_slices() {
    let n = Arc::new(MemNginx::default());
    let b = body(40);
    n.whole(BASE, &b, SLICE);
    let r = reader(&n);
    let whole = r.read(T, None).unwrap().unwrap();
    assert_eq!((whole.start, whole.total, whole.bytes.clone()), (0, 40, b.clone()));
    assert!(String::from_utf8_lossy(&whole.meta).contains("application/octet-stream"), "the stored headers");
    // A range across two slices opens only those.
    n.opened.lock().unwrap().clear();
    let part = r.read(T, Some((10, 20))).unwrap().unwrap();
    assert_eq!((part.start, part.total, part.bytes.as_slice()), (10, 40, &b[10..20]));
    let touched: std::collections::BTreeSet<String> = n.opened.lock().unwrap().iter().cloned().collect();
    assert_eq!(touched.len(), 2, "only the two slices it needs");
    // The last, short slice.
    assert_eq!(r.read(T, Some((33, 40))).unwrap().unwrap().bytes, &b[33..40]);
}

#[test]
fn a_missing_slice_or_an_unusable_answer_reads_nothing() {
    let n = Arc::new(MemNginx::default());
    let b = body(40);
    n.whole(BASE, &b, SLICE);
    n.remove(&slice_key(BASE, 16, SLICE));
    let r = reader(&n);
    assert_eq!(r.read(T, None).unwrap(), None);
    assert_eq!(r.read(T, Some((20, 30))).unwrap(), None);
    assert!(r.read(T, Some((0, 16))).unwrap().is_some(), "the slices it holds still read");
    // A stored 404.
    let key = slice_key(BASE, 32, SLICE);
    n.put(&key, cache_file(&key, "HTTP/1.1 404 Not Found", "Content-Length: 0\r\n", b""));
    assert_eq!(r.read(T, Some((32, 40))).unwrap(), None);
    // Too large to read here, empty and unknown files.
    assert_eq!(r.read(T, Some((0, 2000))).unwrap(), None);
    assert_eq!(r.read(T, Some((5, 5))).unwrap(), None);
    assert_eq!(r.read("/tpr/ovw/data/00/00/none", None).unwrap(), None);
}

#[test]
fn a_slice_that_contradicts_its_key_is_an_error() {
    let n = Arc::new(MemNginx::default());
    let b = body(40);
    n.whole(BASE, &b, SLICE);
    // The file named for slice 16 holds another key.
    let key = slice_key(BASE, 16, SLICE);
    let other = cache_file(
        "blizzard/elsewhere",
        "HTTP/1.1 206 Partial Content",
        "Content-Range: bytes 16-31/40\r\n",
        &b[16..32],
    );
    n.put(&key, other);
    assert!(reader(&n).read(T, None).unwrap_err().contains("stored as"));
    // Another offset, or another total.
    let wrong = |range: &str| {
        cache_file(&key, "HTTP/1.1 206 Partial Content", &format!("Content-Range: {range}\r\n"), &b[16..32])
    };
    n.put(&key, wrong("bytes 0-15/40"));
    assert!(reader(&n).read(T, None).is_err());
    n.put(&key, wrong("bytes 16-31/50"));
    assert!(reader(&n).read(T, None).is_err());
    // A range past the end of the file.
    n.whole(BASE, &b, SLICE);
    assert!(reader(&n).read(T, Some((30, 45))).unwrap_err().contains("40 bytes"));
}

#[test]
fn a_large_file_comes_slice_by_slice() {
    let n = Arc::new(MemNginx::default());
    let b = body(100);
    n.whole(BASE, &b, SLICE);
    let r = reader(&n);
    // Larger than read() takes whole; open() streams it.
    let opts = LancacheOptions { service: "blizzard".into(), slice_bytes: SLICE, max_piece: 50 };
    let small = LancacheRead::new(Arc::clone(&n) as _, opts);
    assert_eq!(small.read(T, None).unwrap(), None, "too large to hold");
    let mut s = small.open(T, None).unwrap().unwrap();
    assert_eq!((s.start, s.end, s.total), (0, 100, 100));
    let mut got = Vec::new();
    let mut parts = 0;
    while let Next::Bytes(at, bytes) = s.next_piece().unwrap() {
        assert_eq!(at, got.len() as u64);
        got.extend_from_slice(&bytes);
        parts += 1;
    }
    assert_eq!((got, parts), (b.clone(), 7), "100 bytes in slices of 16");
    // A range missing any slice is not opened at all.
    n.remove(&slice_key(BASE, 80, SLICE));
    assert!(r.open(T, Some((10, 90))).unwrap().is_none());
    assert!(r.open(T, Some((10, 70))).unwrap().is_some());
}

#[test]
fn a_first_slice_holding_the_whole_file_is_read_alone() {
    // As lancache holds some Steam chunks: the origin answered slice 0's range
    // with the whole file (200), and slice 1 was fetched on its own as well.
    let n = Arc::new(MemNginx::default());
    let b = body(40);
    let k0 = slice_key(BASE, 0, SLICE);
    let len = format!("Content-Length: {}\r\n", b.len());
    n.put(&k0, cache_file(&k0, "HTTP/1.1 200 OK", &len, &b));
    let k1 = slice_key(BASE, 16, SLICE);
    n.put(
        &k1,
        cache_file(
            &k1,
            "HTTP/1.1 206 Partial Content",
            "Content-Length: 16\r\nContent-Range: bytes 16-31/40\r\n",
            &b[16..32],
        ),
    );
    let r = reader(&n);
    let whole = r.read(T, None).unwrap().unwrap();
    assert_eq!((whole.start, whole.total, whole.bytes.clone()), (0, 40, b.clone()), "slice 2 is not needed");
    let mut s = r.open(T, None).unwrap().unwrap();
    assert_eq!(s.next_piece().unwrap(), Next::Bytes(0, b.clone()));
    assert_eq!(s.next_piece().unwrap(), Next::End, "the covered slices are not read again");
    assert_eq!(r.read(T, Some((20, 30))).unwrap().unwrap().bytes, &b[20..30], "from slice 1 itself");
    assert_eq!(r.read(T, Some((20, 35))).unwrap(), None, "a range from slice 1 on needs its own slices");
}

#[test]
fn the_start_of_a_file_lancache_lacks_the_end_of() {
    let n = Arc::new(MemNginx::default());
    let b = body(40);
    n.slice(BASE, &b, 0, SLICE);
    n.slice(BASE, &b, 16, SLICE); // slice 32 never fetched
    let r = reader(&n);
    assert_eq!(r.read(T, None).unwrap(), None, "not all of it");
    let mut s = r.open_prefix(T).unwrap().unwrap();
    assert_eq!((s.start, s.end, s.total), (0, 32, 40));
    assert_eq!(s.next_piece().unwrap(), Next::Bytes(0, b[..16].to_vec()));
    assert_eq!(s.next_piece().unwrap(), Next::Bytes(16, b[16..32].to_vec()));
    assert_eq!(s.next_piece().unwrap(), Next::End);
    n.slice(BASE, &b, 32, SLICE);
    assert!(r.open_prefix(T).unwrap().is_none(), "all of it: open reads it");
    assert!(r.open_prefix("/tpr/ovw/data/00/00/none").unwrap().is_none(), "none of it");
}
