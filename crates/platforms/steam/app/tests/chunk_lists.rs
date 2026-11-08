//! Tests for `lanstash_steam_app::chunk_lists`: lists kept by depot and gid.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash_ports::keep::Keep;
use lanstash_steam_app::chunk_lists::{ChunkLists, decode, encode, name};
use lanstash_steam_domain::manifest::Chunk;

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, Vec<u8>>>);

impl Keep for Memory {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), data.to_vec());
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

fn chunk(sha: u8, compressed: u32) -> Chunk {
    Chunk { sha: [sha; 20], crc: 7, offset: 0, original: compressed * 2, compressed }
}

#[test]
fn a_list_round_trips_in_32_bytes_a_chunk_and_a_broken_one_is_none() {
    let chunks = vec![chunk(1, 100), chunk(2, 200)];
    let b = encode(42, &chunks);
    assert_eq!(b.len(), 16 + 2 * 32);
    assert_eq!(decode(&b), Some((42, chunks)));
    assert_eq!(decode(&b[..b.len() - 1]), None, "cut short");
    assert_eq!(decode(b"LSD2xxxxxxxxxxxx"), None, "another format");
    assert_eq!(name(1_091_501), "steam-depot-1091501");
}

#[test]
fn a_kept_list_serves_only_its_own_manifest_and_a_new_one_replaces_it() {
    let keep: Arc<Memory> = Arc::default();
    let lists = ChunkLists::new(keep.clone());
    assert!(lists.kept(101, 11).is_none());
    lists.put(101, 11, &[chunk(1, 100)]);
    assert_eq!(lists.kept(101, 11), Some(vec![chunk(1, 100)]));
    assert!(lists.kept(101, 12).is_none(), "another build's manifest");
    lists.put(101, 12, &[chunk(3, 300)]);
    assert!(lists.kept(101, 11).is_none(), "replaced");
    assert_eq!(keep.0.lock().unwrap().len(), 1, "one blob per depot");
    assert!(ChunkLists::default().kept(101, 12).is_none(), "nothing kept without a Keep");
}

#[test]
fn a_forgotten_depot_keeps_no_list_and_others_stay() {
    let keep: Arc<Memory> = Arc::default();
    let lists = ChunkLists::new(keep.clone());
    lists.put(101, 11, &[chunk(1, 100)]);
    lists.put(102, 21, &[chunk(2, 200)]);
    lists.forget(101);
    lists.forget(999); // never kept: nothing to do
    assert!(lists.kept(101, 11).is_none());
    assert_eq!(lists.kept(102, 21), Some(vec![chunk(2, 200)]));
    assert_eq!(keep.0.lock().unwrap().len(), 1);
    ChunkLists::default().forget(101); // without a Keep: nothing to do
}
