//! Tests for `lanstash_steam_app::inspect`: one chunk read out of lancache as
//! an import reads it, for each way that can go.

use std::collections::BTreeMap;
use std::sync::Arc;

use lanstash_app::lancache_read::{LancacheOptions, LancacheRead};
use lanstash_domain::checksum::crc::crc32;
use lanstash_domain::nginx_cache::slice_key;
use lanstash_steam_app::inspect::{Outcome, Tally, classify, view};
use lanstash_steam_domain::manifest::{Chunk, chunk_path};
use lanstash_store::test_support::nginx::{MemNginx, cache_file};

const SLICE: u64 = 1 << 20;

fn chunk(n: u8, compressed: u32) -> Chunk {
    Chunk { sha: [n; 20], crc: 0, offset: 0, original: compressed, compressed }
}

/// Stores `body` as chunk `c` of depot 7 in one slice, with `crc` as its x-content-crc (if any).
fn store(n: &MemNginx, c: &Chunk, status: &str, body: &[u8], crc: Option<u32>) {
    let key = slice_key(&format!("steam{}", chunk_path(7, c)), 0, SLICE);
    let mut headers =
        format!("Content-Length: {}\r\nContent-Range: bytes 0-{}/{}\r\n", body.len(), body.len() - 1, body.len());
    if let Some(crc) = crc {
        headers.push_str(&format!("X-Content-Crc: {crc}\r\n"));
    }
    n.put(&key, cache_file(&key, status, &headers, body));
}

#[test]
fn each_way_a_chunk_can_come_out_of_lancache() {
    let n = Arc::new(MemNginx::default());
    let read = LancacheRead::new(
        Arc::clone(&n) as _,
        LancacheOptions { service: "steam".into(), slice_bytes: SLICE, max_piece: 1 << 30 },
    );
    let body = vec![5u8; 100];
    let good = chunk(1, 100);
    store(&n, &good, "HTTP/1.1 206 Partial Content", &body, Some(crc32(&body)));
    let bare = chunk(2, 100);
    store(&n, &bare, "HTTP/1.1 206 Partial Content", &body, None);
    let long = chunk(3, 90);
    store(&n, &long, "HTTP/1.1 206 Partial Content", &body, Some(crc32(&body)));
    let bad = chunk(4, 100);
    store(&n, &bad, "HTTP/1.1 206 Partial Content", &body, Some(crc32(&body) ^ 1));
    let refused = chunk(5, 100);
    store(&n, &refused, "HTTP/1.1 403 Forbidden", &body, None);
    let outcome = |c: &Chunk| classify(&read, n.as_ref(), 7, c).0;
    assert_eq!(outcome(&good), Outcome::Ok);
    assert_eq!(outcome(&chunk(9, 100)), Outcome::Absent);
    assert_eq!(outcome(&bare), Outcome::NoCrc);
    assert_eq!(outcome(&long), Outcome::Size);
    assert!(classify(&read, n.as_ref(), 7, &long).1.ends_with("100 bytes in lancache, 90 in the manifest"));
    assert_eq!(outcome(&bad), Outcome::Crc);
    assert_eq!(outcome(&refused), Outcome::Unusable);
}

#[test]
fn the_view_counts_each_outcome_with_an_example() {
    let mut t: Tally = BTreeMap::new();
    t.insert(Outcome::Ok, (3, 300, "/depot/7/chunk/aa".into()));
    t.insert(Outcome::NoCrc, (1, 100, "/depot/7/chunk/bb".into()));
    assert_eq!(
        view("10", 4, &t),
        "{\"game\":\"10\",\"sampled\":4,\"outcomes\":{\"no_crc\":{\"chunks\":1,\"bytes\":100,\"example\":\"/depot/7/chunk/bb\"},\
         \"ok\":{\"chunks\":3,\"bytes\":300,\"example\":\"/depot/7/chunk/aa\"}}}\n"
    );
}
