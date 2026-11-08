//! Tests for `lanstash_ports::settings`: the port's shape.

use std::sync::Mutex;

use lanstash_ports::settings::Tunables;

#[derive(Default)]
struct One(Mutex<Option<String>>);

impl Tunables for One {
    fn view(&self) -> String {
        format!("{:?}", self.0.lock().unwrap())
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        if key != "a" {
            return Err(format!("`{key}` cannot be changed here"));
        }
        *self.0.lock().unwrap() = Some(value.into());
        Ok(())
    }
    fn reset(&self, _key: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[test]
fn a_setting_is_set_viewed_and_reset() {
    let t = One::default();
    t.set("a", "1").unwrap();
    assert_eq!(t.view(), "Some(\"1\")");
    assert!(t.set("b", "1").is_err());
    t.reset("a").unwrap();
    assert_eq!(t.view(), "None");
}
