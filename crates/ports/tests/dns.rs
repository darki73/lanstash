//! Tests for `lanstash_ports::dns`: the trait works as an object.

use std::sync::{Arc, Mutex};

use lanstash_ports::dns::{DnsConfig, DnsKind, DnsServer, DnsSetup, Lookup, Rewrite};

#[derive(Default)]
struct Memory(Mutex<Vec<Rewrite>>);

impl DnsServer for Memory {
    fn kind(&self) -> &str {
        "memory"
    }
    fn rewrites(&self) -> Result<Vec<Rewrite>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn add(&self, r: &Rewrite) -> Result<(), String> {
        self.0.lock().unwrap().push(r.clone());
        Ok(())
    }
    fn remove(&self, r: &Rewrite) -> Result<(), String> {
        self.0.lock().unwrap().retain(|x| x != r);
        Ok(())
    }
    fn lookups(&self, _limit: usize) -> Result<Vec<Lookup>, String> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct Setup(Mutex<Option<(DnsConfig, String)>>);

impl DnsSetup for Setup {
    fn kinds(&self) -> Vec<DnsKind> {
        vec![DnsKind { id: "memory".into(), name: "Memory".into(), port: 1, https_port: 2, user: true }]
    }
    fn config(&self) -> Option<DnsConfig> {
        self.0.lock().unwrap().as_ref().map(|(c, _)| c.clone())
    }
    fn password(&self) -> Option<String> {
        self.0.lock().unwrap().as_ref().map(|(_, p)| p.clone())
    }
    fn save(&self, config: &DnsConfig, password: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = Some((config.clone(), password.into()));
        Ok(())
    }
    fn forget(&self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
    fn connect(&self, config: &DnsConfig, _password: &str) -> Option<Arc<dyn DnsServer>> {
        (config.kind == "memory").then(|| Arc::new(Memory::default()) as Arc<dyn DnsServer>)
    }
}

#[test]
fn a_setup_is_kept_and_connects_to_its_kind() {
    let s: &dyn DnsSetup = &Setup::default();
    let c = DnsConfig {
        kind: "memory".into(),
        host: "h".into(),
        port: 1,
        https: true,
        pin: Some("AB".into()),
        unchecked: false,
        user: "u".into(),
        answer: None,
        via: Some("cache.example".into()),
    };
    s.save(&c, "pw").unwrap();
    assert_eq!((s.config(), s.password().as_deref()), (Some(c.clone()), Some("pw")));
    assert!(s.connect(&c, "pw").is_some_and(|d| d.kind() == "memory"));
    assert!(s.connect(&DnsConfig { kind: "other".into(), ..c }, "pw").is_none());
    s.forget().unwrap();
    assert!(s.config().is_none() && s.kinds().len() == 1);
}

#[test]
fn rewrites_come_and_go() {
    let dns: &dyn DnsServer = &Memory::default();
    let r = Rewrite { pattern: "*.steamcontent.com".into(), answer: "10.0.0.2".into() };
    dns.add(&r).unwrap();
    assert_eq!(dns.rewrites().unwrap(), std::slice::from_ref(&r));
    dns.remove(&r).unwrap();
    assert!(dns.rewrites().unwrap().is_empty() && dns.lookups(5).unwrap().is_empty());
    assert_eq!(dns.kind(), "memory");
}
