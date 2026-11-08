//! Tests for `lanstash_ports::secrets`: the trait works as an object.

use std::collections::BTreeMap;
use std::sync::Mutex;

use lanstash_ports::secrets::Secrets;

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, String>>);

impl Secrets for Memory {
    fn load(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), value.into());
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

#[test]
fn a_secret_is_kept_replaced_and_forgotten() {
    let s: &dyn Secrets = &Memory::default();
    assert_eq!(s.load("steam"), Ok(None));
    s.save("steam", "a").unwrap();
    s.save("steam", "b").unwrap();
    assert_eq!(s.load("steam"), Ok(Some("b".into())));
    s.remove("steam").unwrap();
    s.remove("steam").unwrap();
    assert_eq!(s.load("steam"), Ok(None));
}
