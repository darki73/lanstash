//! Tests for `lanstash::dns`: the patterns lanstash wants, and the setup kept.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash::dns::{KEPT, LiveDns, PASSWORD, patterns, read, write};
use lanstash_ports::dns::{DnsConfig, DnsSetup};
use lanstash_ports::keep::Keep;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};

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

impl Secrets for Memory {
    fn load(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(name).map(|b| String::from_utf8_lossy(b).into_owned()))
    }
    fn save(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), value.as_bytes().to_vec());
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

/// An AdGuard Home that has no rewrites; keeps what was asked.
#[derive(Default)]
struct Empty(Mutex<Vec<String>>);

impl Web for Empty {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        let tls = req.tls.map_or("http".to_owned(), |t| format!("https pin={:?} unchecked={}", t.pin, t.unchecked));
        self.0.lock().unwrap().push(format!("{} {}:{}{} {tls}", req.method, req.host, req.port, req.target));
        Ok(WebResponse { status: 200, body: b"[]".to_vec() })
    }
}

fn config() -> DnsConfig {
    DnsConfig {
        kind: "adguard".into(),
        host: "192.0.2.53".into(),
        port: 443,
        https: true,
        pin: Some("AB:CD".into()),
        unchecked: false,
        user: "lanstash".into(),
        answer: Some("192.0.2.10".into()),
        via: Some("lancache.example".into()),
    }
}

#[test]
fn the_patterns_are_the_lists_as_written_and_the_built_in_hosts_with_their_subdomains() {
    let steam = "# Steam\nlancache.steamcontent.com\n\n";
    let blizzard = "*.blizzard.com # all of it\nLevel3.Blizzard.com\n*.blizzard.com\n";
    assert_eq!(
        patterns(&[steam, blizzard], &["steampipe.akamaized.net"]),
        [
            "lancache.steamcontent.com",
            "*.blizzard.com",
            "level3.blizzard.com",
            "steampipe.akamaized.net",
            "*.steampipe.akamaized.net"
        ]
    );
}

#[test]
fn a_setup_is_kept_in_lines_and_the_password_apart() {
    assert_eq!(read(&write(&config())), Some(config()));
    assert_eq!(read(&write(&DnsConfig { answer: None, ..config() })).unwrap().answer, None);
    assert_eq!(read(""), None, "a forgotten setup");
    let (keep, secrets, web) = (Arc::new(Memory::default()), Arc::new(Memory::default()), Arc::new(Empty::default()));
    let live = LiveDns::new(keep.clone(), secrets.clone(), web.clone());
    assert!(live.config().is_none() && live.password().is_none());
    live.save(&config(), "s3cret").unwrap();
    assert_eq!((live.config(), live.password().as_deref()), (Some(config()), Some("s3cret")));
    let kept = String::from_utf8(Keep::load(keep.as_ref(), KEPT).unwrap().unwrap()).unwrap();
    assert!(!kept.contains("s3cret"), "the password is not in the setup file: {kept}");
    assert_eq!(Secrets::load(secrets.as_ref(), PASSWORD).unwrap().as_deref(), Some("s3cret"));
    let server = live.connect(&config(), "s3cret").unwrap();
    assert_eq!((server.kind(), server.rewrites().unwrap().len()), ("AdGuard Home", 0));
    assert_eq!(
        web.0.lock().unwrap()[0],
        "GET 192.0.2.53:443/control/rewrite/list https pin=Some(\"AB:CD\") unchecked=false",
        "HTTPS with the pinned certificate, as kept"
    );
    let pihole = live.connect(&DnsConfig { kind: "pihole".into(), ..config() }, "x").unwrap();
    assert_eq!(pihole.kind(), "Pi-hole");
    assert!(live.connect(&DnsConfig { kind: "bind".into(), ..config() }, "x").is_none(), "a kind it does not know");
    live.forget().unwrap();
    assert!(live.config().is_none() && live.password().is_none());
    let kinds: Vec<(String, bool)> = live.kinds().into_iter().map(|k| (k.id, k.user)).collect();
    assert_eq!(kinds, [("adguard".to_owned(), true), ("pihole".to_owned(), false)], "Pi-hole takes a password alone");
}

#[test]
fn a_password_found_beside_the_setup_moves_to_the_secrets() {
    let (keep, secrets, web) = (Arc::new(Memory::default()), Arc::new(Memory::default()), Arc::new(Empty::default()));
    Keep::save(keep.as_ref(), PASSWORD, b"s3cret").unwrap();
    let live = LiveDns::new(keep.clone(), secrets.clone(), web);
    assert_eq!(live.password().as_deref(), Some("s3cret"));
    assert_eq!(Secrets::load(secrets.as_ref(), PASSWORD).unwrap().as_deref(), Some("s3cret"));
    assert_eq!(Keep::load(keep.as_ref(), PASSWORD).unwrap(), None, "not left beside the setup");
}
