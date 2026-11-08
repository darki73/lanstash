//! Tests for `lanstash_steam_domain::manifest`, on a manifest built here with
//! the section magics and field numbers of a real one, zipped as stored.

use lanstash_domain::checksum::crc::crc32;
use lanstash_steam_domain::cm::proto::Writer;
use lanstash_steam_domain::manifest::{Chunk, ManifestError, chunk_path, parse, parse_sections};

fn chunk(sha: u8, compressed: u32) -> Vec<u8> {
    Writer::new()
        .bytes(1, &[sha; 20])
        .fixed32(2, 0xDEAD_BEEF)
        .varint(3, 0)
        .varint(4, u64::from(compressed) * 3)
        .varint(5, u64::from(compressed))
        .finish()
}

fn sections() -> Vec<u8> {
    let file = |name: &str, size: u64, chunks: &[Vec<u8>]| {
        let mut w = Writer::new().string(1, name).varint(2, size).varint(3, 0);
        for c in chunks {
            w = w.bytes(6, c);
        }
        w.finish()
    };
    let payload = Writer::new()
        .bytes(1, &file("a.bin", 300, &[chunk(1, 100), chunk(2, 50)]))
        .bytes(1, &file("b.bin", 150, &[chunk(1, 100)])) // the same chunk again
        .finish();
    let metadata =
        Writer::new().varint(1, 1_422_451).varint(2, 99).varint(3, 1_700_000_000).bool(4, true).varint(7, 2).finish();
    let mut out = Vec::new();
    for (magic, body) in [(0x71F6_17D0u32, payload), (0x1F48_12BE, metadata), (0x1B81_B817, vec![1, 2, 3])] {
        out.extend_from_slice(&magic.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend(body);
    }
    out.extend_from_slice(&0x32C4_15ABu32.to_le_bytes());
    out
}

/// A ZIP of one stored entry `z`.
fn zip(data: &[u8]) -> Vec<u8> {
    let crc = crc32(data);
    let len = data.len() as u32;
    let mut z = Vec::new();
    z.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    z.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&[1, 0, 0, 0, b'z']);
    z.extend_from_slice(data);
    let central = z.len() as u32;
    z.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    z.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.push(b'z');
    let size = z.len() as u32 - central;
    z.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    z.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    z.extend_from_slice(&size.to_le_bytes());
    z.extend_from_slice(&central.to_le_bytes());
    z.extend_from_slice(&[0, 0]);
    z
}

#[test]
fn a_manifest_lists_its_files_and_unique_chunks() {
    let m = parse(&zip(&sections())).unwrap();
    assert_eq!(
        (m.depot, m.gid, m.created, m.names_encrypted, m.unique_chunks),
        (1_422_451, 99, 1_700_000_000, true, 2)
    );
    assert_eq!(
        m.files.iter().map(|f| (f.name.as_str(), f.size, f.chunks.len())).collect::<Vec<_>>(),
        [("a.bin", 300, 2), ("b.bin", 150, 1)]
    );
    let chunks = m.chunks();
    assert_eq!(chunks.len(), 2, "a chunk two files share is fetched once");
    assert_eq!(m.download(), 150);
    assert_eq!(chunks[0], Chunk { sha: [1; 20], crc: 0xDEAD_BEEF, offset: 0, original: 300, compressed: 100 });
    assert_eq!(chunk_path(m.depot, &chunks[1]), format!("/depot/1422451/chunk/{}", "02".repeat(20)));
}

#[test]
fn broken_manifests_are_errors() {
    let s = sections();
    assert_eq!(parse_sections(&s[..s.len() - 2]), Err(ManifestError::Sections));
    let mut unknown = s.clone();
    unknown[0] ^= 1;
    assert_eq!(parse_sections(&unknown), Err(ManifestError::Sections));
    assert!(matches!(parse(b"not a zip"), Err(ManifestError::Zip(_))));
}
