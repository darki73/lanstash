//! Tests for `lanstash_ports::link`: the traits work as objects.

use lanstash_ports::link::{Dialer, Link};

struct Echo(Vec<Vec<u8>>);

impl Link for Echo {
    fn send(&mut self, m: &[u8]) -> Result<(), String> {
        self.0.push(m.to_vec());
        Ok(())
    }
    fn recv(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.pop())
    }
}

struct Local;

impl Dialer for Local {
    fn dial(&self, _: &str, _: u16, _: &str) -> Result<Box<dyn Link>, String> {
        Ok(Box::new(Echo(Vec::new())))
    }
    fn get(&self, host: &str, path: &str) -> Result<Vec<u8>, String> {
        Ok(format!("{host}{path}").into_bytes())
    }
}

#[test]
fn a_dialed_link_carries_messages_and_says_when_none_came() {
    let d: &dyn Dialer = &Local;
    let mut l = d.dial("cm.example", 443, "/cmsocket/").unwrap();
    assert_eq!(l.recv(), Ok(None));
    l.send(b"hello").unwrap();
    assert_eq!(l.recv(), Ok(Some(b"hello".to_vec())));
    assert_eq!(d.get("api.example", "/x").unwrap(), b"api.example/x");
}
