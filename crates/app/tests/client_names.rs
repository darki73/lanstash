//! Tests for `lanstash_app::client_names`: a given name wins, else the network's,
//! looked up off the request path and kept as long as the answer says.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_app::client_names::{ClientName, ClientNames, MAX_LABEL, MIN_KEEP_MS, clean_label};
use lanstash_ports::clients::{ClientLabels, ReverseNames};
use lanstash_ports::clock::Clock;

struct Ticking(AtomicU64);

impl Clock for Ticking {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// The network: answers from `hosts`, failing while `down`; counts what it is asked.
#[derive(Default)]
struct Network {
    hosts: Mutex<BTreeMap<IpAddr, String>>,
    down: Mutex<bool>,
    asked: AtomicUsize,
}

impl ReverseNames for Network {
    fn names_of(&self, ip: IpAddr) -> Result<(Vec<String>, u32), String> {
        self.asked.fetch_add(1, Ordering::Relaxed);
        if *self.down.lock().unwrap() {
            return Err("no answer".into());
        }
        Ok((self.hosts.lock().unwrap().get(&ip).cloned().into_iter().collect(), 30))
    }
}

#[derive(Default)]
struct Labels(Mutex<BTreeMap<String, String>>);

impl ClientLabels for Labels {
    fn labels(&self) -> Result<Vec<(String, String)>, String> {
        Ok(self.0.lock().unwrap().iter().map(|(a, n)| (a.clone(), n.clone())).collect())
    }
    fn set_label(&self, address: &str, label: Option<&str>) -> Result<(), String> {
        let mut m = self.0.lock().unwrap();
        match label {
            Some(l) => m.insert(address.into(), l.into()),
            None => m.remove(address),
        };
        Ok(())
    }
}

fn rig() -> (Arc<Network>, Arc<Labels>, Arc<Ticking>, ClientNames) {
    let net = Arc::new(Network::default());
    net.hosts.lock().unwrap().insert("10.0.0.5".parse().unwrap(), "Game-PC.".into());
    let labels = Arc::new(Labels::default());
    let clock = Arc::new(Ticking(AtomicU64::new(1_000)));
    let names = ClientNames::new(labels.clone(), Some(net.clone()), clock.clone()).unwrap();
    (net, labels, clock, names)
}

#[test]
fn the_networks_name_is_looked_up_once_and_kept() {
    let (net, _, clock, names) = rig();
    let five = "10.0.0.5".to_string();
    assert_eq!(names.name(&five), ClientName::default(), "nothing until looked up");
    names.look_up(&[five.clone(), "10.0.0.9".into(), "not an address".into()]);
    assert_eq!(names.name(&five).host.as_deref(), Some("Game-PC"), "as written, without the trailing dot");
    assert_eq!(names.name("10.0.0.9").host, None);
    assert_eq!(net.asked.load(Ordering::Relaxed), 2);
    names.look_up(std::slice::from_ref(&five));
    assert_eq!(net.asked.load(Ordering::Relaxed), 2, "a 30 s answer is kept the least time");
    clock.0.store(1_000 + MIN_KEEP_MS, Ordering::Relaxed);
    *net.down.lock().unwrap() = true;
    names.look_up(std::slice::from_ref(&five));
    assert_eq!(net.asked.load(Ordering::Relaxed), 3);
    assert_eq!(names.name(&five).host.as_deref(), Some("Game-PC"), "a failed lookup keeps what was known");
}

#[test]
fn a_given_name_wins_is_kept_and_can_be_taken_away() {
    let (_, labels, _, names) = rig();
    names.look_up(&["10.0.0.5".to_string()]);
    names.set_label("::ffff:10.0.0.5", "  Living room\t").unwrap();
    assert_eq!(
        names.name("10.0.0.5"),
        ClientName { label: Some("Living room".into()), host: Some("Game-PC".into()) },
        "trimmed, without control characters, under the IPv4 address"
    );
    assert_eq!(labels.labels().unwrap(), [("10.0.0.5".to_string(), "Living room".to_string())]);
    names.set_label("10.0.0.5", " ").unwrap();
    assert_eq!(names.name("10.0.0.5").label, None);
    assert!(labels.labels().unwrap().is_empty());
    assert!(names.set_label("pc", "x").is_err(), "not an address");
    assert!(names.set_label("10.0.0.5", &"x".repeat(MAX_LABEL + 1)).is_err());
}

#[test]
fn names_given_before_are_known_at_the_start() {
    let labels = Arc::new(Labels::default());
    labels.set_label("10.0.0.7", Some("Office")).unwrap();
    let names = ClientNames::new(labels, None, Arc::new(Ticking(AtomicU64::new(1)))).unwrap();
    assert_eq!(names.name("10.0.0.7").label.as_deref(), Some("Office"));
    names.look_up(&["10.0.0.7".to_string()]);
    assert_eq!(names.name("10.0.0.7").host, None, "no way to look it up: never");
    assert!(Arc::new(names).keep_going(Arc::new(Vec::new), Arc::new(lanstash_ports::clock::NoPause)).is_ok());
}

#[test]
fn a_label_is_cleaned() {
    assert_eq!(clean_label(" a\u{7}b "), Ok(Some("ab".to_string())));
    assert_eq!(clean_label(""), Ok(None));
    assert_eq!(clean_label(&"é".repeat(MAX_LABEL)).map(|l| l.map(|s| s.chars().count())), Ok(Some(MAX_LABEL)));
}
