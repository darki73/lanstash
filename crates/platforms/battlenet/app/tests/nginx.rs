//! The nginx probe: which slice files of lancache's cache a Battle.net build's
//! plan needs, and how much of the build they hold.

mod support;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use lanstash_battlenet_app::nginx::{NginxProbe, ProbeOptions};
use lanstash_battlenet_domain::nginx_keys::{Tally, wanted};
use lanstash_battlenet_domain::tact::Version;
use lanstash_domain::nginx_cache::cache_file_path;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreError, StoreErrorKind};
use support::*;

/// An nginx cache that only knows which files exist (and counts the asks).
#[derive(Default)]
struct Present {
    files: BTreeSet<String>,
    asked: Mutex<u64>,
    broken: bool,
}

impl NginxCacheSource for Present {
    fn list_files(&self, _: &[String]) -> Result<Vec<String>, StoreError> {
        Ok(self.files.iter().cloned().collect())
    }
    fn open(&self, rel: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        Err(StoreError::new(StoreErrorKind::NotFound, rel))
    }
    fn exists(&self, rel: &str) -> Result<bool, StoreError> {
        *self.asked.lock().unwrap() += 1;
        if self.broken {
            return Err(StoreError::other("share gone"));
        }
        Ok(self.files.contains(rel))
    }
}

fn version() -> Version {
    Version { build_config: BUILD.into(), cdn_config: CDN_CONFIG.into(), name: "3.3.7".into() }
}

const MIB: u64 = 1 << 20;

#[test]
fn every_piece_of_a_plan_names_its_slices() {
    let w = bnet(false);
    let plan = w.plans.plan("osi", &version()).unwrap();
    let want = wanted(&plan, "blizzard", MIB);
    assert_eq!(want.whole.len(), plan.files.len());
    assert_eq!(want.archived.len(), plan.parts.len());
    // A small whole file is one slice, keyed as lancache keys it.
    let (size, keys) = &want.whole[0];
    assert!(*size < MIB);
    assert_eq!(keys.len(), 1);
    assert_eq!(want.keys[keys[0]], format!("blizzard{}bytes=0-1048575", at("config", BUILD, "")));
    // Files inside one archive share its slices: each slice key once.
    let distinct: BTreeSet<&String> = want.keys.iter().collect();
    assert_eq!(distinct.len(), want.keys.len());
    let archive = format!("blizzard{}bytes=0-1048575", at("data", ARCHIVE_1, ""));
    assert!(want.keys.contains(&archive));
}

#[test]
fn the_probe_counts_what_the_nginx_cache_holds() {
    let w = bnet(false);
    let plan = w.plans.plan("osi", &version()).unwrap();
    let want = wanted(&plan, "blizzard", MIB);
    // lancache holds every slice but the first whole file's.
    let first = want.whole[0].1[0];
    let files = want.keys.iter().enumerate().filter(|(i, _)| *i != first).map(|(_, k)| cache_file_path(k)).collect();
    let source = Arc::new(Present { files, ..Present::default() });
    let probe =
        NginxProbe::new(Arc::clone(&w.builds), source.clone(), ProbeOptions { threads: 3, ..ProbeOptions::default() });
    let r = probe.probe("osi").unwrap();
    assert_eq!((r.game.as_str(), r.product.as_str()), ("osi", "osi"));
    assert_eq!(r.slices, want.keys.len() as u64);
    assert_eq!(r.slices_found, r.slices - 1);
    assert_eq!(*source.asked.lock().unwrap(), r.slices);
    let bytes = |p: &[(u64, Vec<usize>)]| p.iter().map(|x| x.0).sum::<u64>();
    assert_eq!(
        r.whole,
        Tally {
            wanted: want.whole.len() as u64,
            found: want.whole.len() as u64 - 1,
            bytes: bytes(&want.whole),
            bytes_found: bytes(&want.whole) - want.whole[0].0,
        }
    );
    assert_eq!(r.archived.found, r.archived.wanted);
    assert_eq!(r.missing, vec![(want.keys[first].clone(), cache_file_path(&want.keys[first]))]);
}

#[test]
fn an_unreadable_cache_or_unknown_game_is_an_error() {
    let w = bnet(false);
    let broken = Arc::new(Present { broken: true, ..Present::default() });
    let probe = NginxProbe::new(Arc::clone(&w.builds), broken, ProbeOptions::default());
    assert!(probe.probe("osi").unwrap_err().contains("share gone"));
    let probe = NginxProbe::new(Arc::clone(&w.builds), Arc::new(Present::default()), ProbeOptions::default());
    assert!(probe.probe("nosuchgame").is_err());
}

#[test]
fn the_share_is_probed_in_the_background_once_per_build() {
    let w = bnet(false);
    let plan = w.plans.plan("osi", &version()).unwrap();
    let want = wanted(&plan, "blizzard", MIB);
    let files = want.keys.iter().map(|k| cache_file_path(k)).collect();
    let source = Arc::new(Present { files, ..Present::default() });
    let probe = Arc::new(NginxProbe::new(Arc::clone(&w.builds), source.clone(), ProbeOptions::default()));
    let config = w.builds.check("osi").build.unwrap().version.build_config;
    assert_eq!(probe.share("osi", &config), None, "not probed yet: queued");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let share = loop {
        if let Some(s) = probe.share("osi", &config) {
            break s;
        }
        assert!(std::time::Instant::now() < until, "the background probe never finished");
        std::thread::yield_now();
    };
    let all: u64 = want.whole.iter().chain(&want.archived).map(|p| p.0).sum();
    assert_eq!((share.have, share.want), (all, all), "lancache holds all of it");
    let asked = *source.asked.lock().unwrap();
    assert_eq!(probe.share("osi", &config).map(|s| s.have), Some(all));
    assert_eq!(*source.asked.lock().unwrap(), asked, "kept: not probed again");
}
