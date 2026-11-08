//! Tests for `lanstash_epic::manifest`, on a manifest and a chunk laid out as the module says.

use lanstash_domain::checksum::sha1::sha1;
use lanstash_epic::manifest::{Chunk, check_chunk, parse};

/// A manifest of `chunks` (uncompressed), feature level 18.
fn manifest(chunks: &[Chunk]) -> Vec<u8> {
    let mut meta = Vec::new();
    meta.push(0u8); // version
    meta.extend_from_slice(&18u32.to_le_bytes()); // feature level
    meta.extend_from_slice(b"rest of the meta");
    let mut data = Vec::new();
    data.extend_from_slice(&(4 + meta.len() as u32).to_le_bytes());
    data.extend_from_slice(&meta);
    let mut list = vec![0u8];
    list.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    chunks.iter().for_each(|c| c.guid.iter().for_each(|g| list.extend_from_slice(&g.to_le_bytes())));
    chunks.iter().for_each(|c| list.extend_from_slice(&c.hash.to_le_bytes()));
    chunks.iter().for_each(|c| list.extend_from_slice(&c.sha1));
    chunks.iter().for_each(|c| list.push(c.group));
    chunks.iter().for_each(|_| list.extend_from_slice(&1_048_576u32.to_le_bytes()));
    chunks.iter().for_each(|c| list.extend_from_slice(&c.file_size.to_le_bytes()));
    data.extend_from_slice(&(4 + list.len() as u32).to_le_bytes());
    data.extend_from_slice(&list);
    data.extend_from_slice(b"the file list");
    let mut b = Vec::new();
    b.extend_from_slice(&0x44BE_C00Cu32.to_le_bytes());
    b.extend_from_slice(&41u32.to_le_bytes());
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(&sha1(&data));
    b.push(0); // stored as is
    b.extend_from_slice(&21u32.to_le_bytes()); // version
    b.extend_from_slice(&data);
    b
}

fn chunk_file(data: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&0xB1FE_3AA2u32.to_le_bytes());
    b.extend_from_slice(&3u32.to_le_bytes());
    b.extend_from_slice(&41u32.to_le_bytes());
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(&[0; 16]);
    b.extend_from_slice(&0u64.to_le_bytes());
    b.push(0);
    b.extend_from_slice(data);
    b
}

#[test]
fn a_manifest_gives_its_chunks_and_where_they_are() {
    let data = b"some chunk data";
    let c = Chunk {
        guid: [1, 0xABCD, 3, 0xFFFF_FFFF],
        hash: 0x0123_4567_89AB_CDEF,
        sha1: sha1(data),
        group: 7,
        file_size: 57,
    };
    let m = parse(&manifest(std::slice::from_ref(&c))).unwrap();
    assert_eq!((m.feature_level, m.chunks.clone()), (18, vec![c.clone()]));
    assert_eq!(c.path(m.feature_level), "ChunksV4/07/0123456789ABCDEF_000000010000ABCD00000003FFFFFFFF.chunk");
    assert_eq!(c.path(5), "ChunksV2/07/0123456789ABCDEF_000000010000ABCD00000003FFFFFFFF.chunk");
    assert!(check_chunk(&chunk_file(data), &c.sha1), "the chunk's data is the manifest's");
    assert!(!check_chunk(&chunk_file(b"other data"), &c.sha1));
    assert!(!check_chunk(b"short", &c.sha1));
}

#[test]
fn what_is_not_a_good_binary_manifest_says_so() {
    let mut bad = manifest(&[]);
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(parse(&bad).unwrap_err().contains("SHA-1"));
    assert!(parse(b"{\"ManifestFileVersion\":\"x\"}").unwrap_err().contains("JSON"));
    assert!(parse(b"nope").is_err());
}
