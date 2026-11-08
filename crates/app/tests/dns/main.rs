//! Tests for `lanstash_app::dns`, against a DNS server in memory that behaves
//! as AdGuard Home does: adding a rewrite that is there
//! makes a second copy; removing one removes every copy.

mod covered;
mod setup;
#[path = "../support/mod.rs"]
mod support;

use std::sync::{Arc, Mutex};

use lanstash_app::dns::{AnswerFrom, DnsLink, NOT_SET_UP};
use lanstash_app::serve::{Cache, ServeOptions};
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::dns::{Answered, DnsConfig, DnsKind, DnsServer, DnsSetup, Lookup, Rewrite};
use lanstash_ports::events::NoEvents;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

#[derive(Default)]
struct Server {
    rewrites: Mutex<Vec<Rewrite>>,
    lookups: Vec<Lookup>,
    calls: Mutex<Vec<String>>,
    /// How names are answered besides the rewrites (by rules); none: it cannot say.
    answers: Option<Vec<(String, Answered)>>,
}

impl DnsServer for Server {
    fn kind(&self) -> &str {
        "test server"
    }
    fn rewrites(&self) -> Result<Vec<Rewrite>, String> {
        Ok(self.rewrites.lock().unwrap().clone())
    }
    fn add(&self, r: &Rewrite) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("add {}", r.pattern));
        self.rewrites.lock().unwrap().push(r.clone());
        Ok(())
    }
    fn remove(&self, r: &Rewrite) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("remove {}", r.pattern));
        self.rewrites.lock().unwrap().retain(|x| x != r);
        Ok(())
    }
    fn lookups(&self, _limit: usize) -> Result<Vec<Lookup>, String> {
        Ok(self.lookups.clone())
    }
    fn check(&self, name: &str) -> Option<Result<Answered, String>> {
        let answers = self.answers.as_ref()?;
        self.calls.lock().unwrap().push(format!("check {name}"));
        let found = answers.iter().find(|(n, _)| n == name).map(|(_, a)| a.clone());
        Some(found.ok_or_else(|| "not answered".to_owned()))
    }
}

fn rw(pattern: &str, answer: &str) -> Rewrite {
    Rewrite { pattern: pattern.into(), answer: answer.into() }
}

fn look(name: &str, client: &str, rewritten: bool, at_ms: u64) -> Lookup {
    Lookup { name: name.into(), client: client.into(), answers: vec![], rewritten, at_ms }
}

fn hosts() -> HostRules {
    let mut h = HostRules::default();
    h.add_list("steam", "lancache.steamcontent.com\n*.steamcontent.com\n");
    h.add_list("blizzard", "*.blizzard.com\nlevel3.blizzard.com\n");
    h
}

fn cache() -> Arc<Cache> {
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(0)))
            .unwrap();
    let profiles = support::profiles::profiles();
    Arc::new(Cache::new(Arc::new(store), hosts(), &profiles, Arc::new(NoEvents), ServeOptions::default()))
}

const WANTED: [&str; 4] = ["lancache.steamcontent.com", "*.steamcontent.com", "*.blizzard.com", "level3.blizzard.com"];

/// A setup in memory whose one kind (`test`) connects to `server`.
struct Setup {
    kept: Mutex<Option<(DnsConfig, String)>>,
    server: Arc<Server>,
}

impl DnsSetup for Setup {
    fn kinds(&self) -> Vec<DnsKind> {
        vec![DnsKind { id: "test".into(), name: "Test".into(), port: 3000, https_port: 443, user: true }]
    }
    fn config(&self) -> Option<DnsConfig> {
        self.kept.lock().unwrap().as_ref().map(|(c, _)| c.clone())
    }
    fn password(&self) -> Option<String> {
        self.kept.lock().unwrap().as_ref().map(|(_, p)| p.clone())
    }
    fn save(&self, config: &DnsConfig, password: &str) -> Result<(), String> {
        *self.kept.lock().unwrap() = Some((config.clone(), password.into()));
        Ok(())
    }
    fn forget(&self) -> Result<(), String> {
        *self.kept.lock().unwrap() = None;
        Ok(())
    }
    fn connect(&self, config: &DnsConfig, password: &str) -> Option<Arc<dyn DnsServer>> {
        if password == "wrong" {
            return Some(Arc::new(Refusing));
        }
        (config.kind == "test").then(|| Arc::clone(&self.server) as Arc<dyn DnsServer>)
    }
}

/// A server that refuses lanstash's password.
struct Refusing;

impl DnsServer for Refusing {
    fn kind(&self) -> &str {
        "refusing"
    }
    fn rewrites(&self) -> Result<Vec<Rewrite>, String> {
        Err("refused the password".into())
    }
    fn add(&self, _: &Rewrite) -> Result<(), String> {
        Err("refused".into())
    }
    fn remove(&self, _: &Rewrite) -> Result<(), String> {
        Err("refused".into())
    }
    fn lookups(&self, _: usize) -> Result<Vec<Lookup>, String> {
        Err("refused".into())
    }
}

fn config(answer: Option<&str>) -> DnsConfig {
    DnsConfig {
        kind: "test".into(),
        host: "10.0.0.53".into(),
        port: 3000,
        https: false,
        pin: None,
        unchecked: false,
        user: "lanstash".into(),
        answer: answer.map(str::to_owned),
        via: None,
    }
}

/// A link to `server`, set up with `answer` (or not set up at all with `None` for `setup`).
fn link_with(server: Arc<Server>, setup: Option<Option<&str>>) -> DnsLink {
    let kept = setup.map(|answer| (config(answer), "pw".to_owned()));
    let wanted: Vec<String> = WANTED.iter().map(|s| (*s).to_owned()).chain(["*.blizzard.com".to_owned()]).collect();
    DnsLink::new(Arc::new(Setup { kept: Mutex::new(kept), server }), cache(), hosts(), wanted)
}

fn link(server: Arc<Server>, answer: Option<&str>) -> DnsLink {
    link_with(server, Some(answer))
}

#[test]
fn plans_from_the_address_the_rewrites_already_answer_and_leaves_others_alone() {
    let server = Arc::new(Server::default());
    *server.rewrites.lock().unwrap() = vec![
        rw("lancache.steamcontent.com", "10.0.0.10"),
        rw("*.steamcontent.com", "10.0.0.10"),
        rw("*.steamcontent.com", "10.0.0.10"),
        rw("cache.lan", "10.0.0.10"),
        rw("printer.lan", "10.0.0.5"),
    ];
    let l = link(Arc::clone(&server), None);
    let plan = l.report().unwrap().plan.unwrap();
    assert_eq!((plan.answer.as_str(), plan.answer_from, plan.wanted), ("10.0.0.10", AnswerFrom::Rewrites, 4));
    assert_eq!(plan.missing, ["*.blizzard.com", "level3.blizzard.com"]);
    assert_eq!(plan.duplicates, ["*.steamcontent.com"]);
    assert_eq!(plan.extra, ["cache.lan"], "lanstash's own name: reported, never removed");
    let done = l.apply().unwrap();
    assert_eq!((done.added, done.deduplicated), (2, 1));
    assert_eq!(
        *server.calls.lock().unwrap(),
        ["remove *.steamcontent.com", "add *.steamcontent.com", "add *.blizzard.com", "add level3.blizzard.com"]
    );
    let after = l.report().unwrap().plan.unwrap();
    assert!(after.missing.is_empty() && after.duplicates.is_empty(), "{after:?}");
    assert!(server.rewrites.lock().unwrap().contains(&rw("printer.lan", "10.0.0.5")), "another address: untouched");
    assert!(l.apply().is_ok_and(|d| d.added == 0 && d.deduplicated == 0), "nothing more to do");
}

#[test]
fn rewrites_with_names_that_are_not_ascii_are_read_without_failing() {
    let server = Arc::new(Server::default());
    *server.rewrites.lock().unwrap() = vec![rw("*.b", "10.0.0.1"), rw("lancache.steamcontent.com", "a\u{e9}")];
    let l = link(Arc::clone(&server), Some("10.0.0.10"));
    assert!(l.report().unwrap().plan.is_ok());
}

#[test]
fn rewrites_that_answer_with_a_name_count_and_new_ones_answer_the_same_way_unless_told() {
    // lanstash's names answer with the host's name, which answers its address,
    // and a wildcard of the same domain sends everything else elsewhere.
    let setup = || {
        vec![
            rw("cache.lan", "10.0.0.10"),
            rw("*.lan", "10.0.50.200"),
            rw("lancache.steamcontent.com", "cache.lan"),
            rw("level3.blizzard.com", "cache.lan"),
            rw("*.blizzard.com", "cache.lan"),
            rw("printer.lan", "10.0.0.5"),
        ]
    };
    let server = Arc::new(Server::default());
    *server.rewrites.lock().unwrap() = setup();
    let inferred = link(Arc::clone(&server), None).report().unwrap().plan.unwrap();
    assert_eq!((inferred.answer.as_str(), inferred.answer_from), ("10.0.0.10", AnswerFrom::Rewrites));
    assert_eq!(inferred.write_as, "cache.lan", "new ones answer the way the existing ones do");
    assert_eq!(inferred.missing, ["*.steamcontent.com"]);
    let service = |p: &str| inferred.services.iter().find(|(n, _)| n == p).map(|(_, s)| s.as_str());
    assert_eq!(
        (service("*.blizzard.com"), service("lancache.steamcontent.com"), inferred.services.len()),
        (Some("blizzard"), Some("steam"), 4),
        "each name with its service"
    );
    assert!(inferred.extra.is_empty(), "lanstash's own name is not a stray: {:?}", inferred.extra);
    link(Arc::clone(&server), None).apply().unwrap();
    assert!(server.rewrites.lock().unwrap().contains(&rw("*.steamcontent.com", "cache.lan")));
    // Told to answer with the address, or with the name.
    *server.rewrites.lock().unwrap() = setup();
    let by_address = link(Arc::clone(&server), Some("10.0.0.10")).report().unwrap().plan.unwrap();
    assert_eq!((by_address.write_as.as_str(), by_address.missing.len()), ("10.0.0.10", 1));
    let by_name = link(Arc::clone(&server), Some("cache.lan")).report().unwrap().plan.unwrap();
    assert_eq!((by_name.answer.as_str(), by_name.write_as.as_str()), ("10.0.0.10", "cache.lan"));
    let nowhere = link(Arc::clone(&server), Some("nowhere.example")).report().unwrap().plan.unwrap_err();
    assert!(nowhere.contains("does not lead to an address"), "{nowhere}");
}

#[test]
fn names_pointed_through_a_name_get_that_name_and_apply_adds_it_when_it_has_no_rewrite() {
    let server = Arc::new(Server::default());
    let via = |v: &str| {
        let s = Setup { kept: Mutex::new(None), server: Arc::clone(&server) };
        let c = DnsConfig { via: Some(v.into()), ..config(Some("10.0.0.10")) };
        s.save(&c, "pw").unwrap();
        let wanted = WANTED.iter().map(|s| (*s).to_owned()).collect();
        DnsLink::new(Arc::new(s), cache(), hosts(), wanted)
    };
    // A fresh server: the name gets its rewrite first, then every game name points at it.
    let l = via("cache.lan");
    let plan = l.report().unwrap().plan.unwrap();
    assert_eq!((plan.write_as.as_str(), plan.via_missing, plan.missing.len()), ("cache.lan", true, 4));
    l.apply().unwrap();
    let calls = server.calls.lock().unwrap().clone();
    assert_eq!(calls[0], "add cache.lan", "the name first");
    assert!(server.rewrites.lock().unwrap().contains(&rw("level3.blizzard.com", "cache.lan")));
    let after = l.report().unwrap().plan.unwrap();
    assert!(!after.via_missing && after.missing.is_empty() && after.extra.is_empty(), "{after:?}");
    // A name that already leads elsewhere is said, and nothing changes.
    *server.rewrites.lock().unwrap() = vec![rw("cache.lan", "10.0.0.99")];
    let wrong = via("cache.lan").report().unwrap().plan.unwrap_err();
    assert!(wrong.contains("leads to 10.0.0.99, not to lanstash's 10.0.0.10"), "{wrong}");
}

#[test]
fn a_configured_address_wins_and_none_at_all_is_said() {
    let server = Arc::new(Server::default());
    *server.rewrites.lock().unwrap() = vec![rw("*.steamcontent.com", "10.0.0.99")];
    let plan = link(Arc::clone(&server), Some("10.0.0.10")).report().unwrap().plan.unwrap();
    assert_eq!((plan.answer_from, plan.missing.len()), (AnswerFrom::Config, 4), "the old address is not lanstash's");
    let none = link(Arc::new(Server::default()), None);
    assert!(none.report().unwrap().plan.unwrap_err().contains("set lanstash's address"));
    assert!(none.apply().is_err());
    let unset = link_with(Arc::new(Server::default()), None);
    assert_eq!(unset.report().unwrap_err(), NOT_SET_UP);
    assert_eq!(unset.apply().unwrap_err(), NOT_SET_UP);
}

#[test]
fn says_which_cached_names_went_to_the_internet_and_which_hosts_lanstash_refused() {
    let server = Server {
        lookups: vec![
            look("cache8-sgp1.steamcontent.com", "10.0.1.100", true, 30),
            look("level3.blizzard.com", "10.0.1.100", false, 20),
            look("Level3.blizzard.com", "10.0.1.101", false, 10),
            look("example.org", "10.0.1.100", false, 5),
            look("eu.cdn.blizzard.com", "10.0.1.100", false, 0),
        ],
        ..Server::default()
    };
    let l = link(Arc::new(server), Some("10.0.0.10"));
    let report = l.report().unwrap();
    assert_eq!((report.kind.as_str(), report.lookups, report.oldest_ms), ("test server", 5, Some(5)));
    let missed: Vec<(&str, &str, u64, usize)> =
        report.missed.iter().map(|m| (m.name.as_str(), m.service.as_str(), m.count, m.clients.len())).collect();
    assert_eq!(missed, [("level3.blizzard.com", "blizzard", 2, 2), ("eu.cdn.blizzard.com", "blizzard", 1, 1)]);
    assert!(report.refused.is_empty() && report.lookups_error.is_none());
}
