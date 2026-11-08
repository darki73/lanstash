//! Tests for `lanstash_ports::clients`: the contracts as minimal implementations.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Mutex;

use lanstash_ports::clients::{ClientLabels, ReverseNames};

struct Hosts(BTreeMap<IpAddr, &'static str>);

impl ReverseNames for Hosts {
    fn names_of(&self, ip: IpAddr) -> Result<(Vec<String>, u32), String> {
        Ok((self.0.get(&ip).map(|n| (*n).to_string()).into_iter().collect(), 60))
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

#[test]
fn an_address_the_network_knows_has_its_names() {
    let ip: IpAddr = "10.0.0.5".parse().unwrap();
    let hosts: &dyn ReverseNames = &Hosts([(ip, "Game-PC")].into());
    assert_eq!(hosts.names_of(ip), Ok((vec!["Game-PC".to_string()], 60)));
    assert_eq!(hosts.names_of("10.0.0.6".parse().unwrap()).unwrap().0, Vec::<String>::new());
}

#[test]
fn a_label_is_given_and_taken_away() {
    let labels: &dyn ClientLabels = &Labels::default();
    labels.set_label("10.0.0.5", Some("Living room")).unwrap();
    assert_eq!(labels.labels().unwrap(), [("10.0.0.5".to_string(), "Living room".to_string())]);
    labels.set_label("10.0.0.5", None).unwrap();
    assert!(labels.labels().unwrap().is_empty());
}
