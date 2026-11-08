//! Tests for `lanstash_ports::keep`: the trait works as an object.

use std::collections::BTreeMap;
use std::sync::Mutex;

use lanstash_ports::keep::Keep;

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
}

#[test]
fn a_blob_is_kept_and_replaced() {
    let k: &dyn Keep = &Memory::default();
    assert_eq!(k.load("x"), Ok(None));
    k.save("x", b"one").unwrap();
    k.save("x", b"two").unwrap();
    assert_eq!(k.load("x"), Ok(Some(b"two".to_vec())));
}
