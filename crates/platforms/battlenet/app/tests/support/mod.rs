#![allow(dead_code)] // each test binary uses a different part
//! A small Battle.net CDN for the builds, plans and prefill tests: product
//! `osi`, region `eu`, every file built here in the CDN's layouts.
//!
//! The build: two archives, a file index, a download manifest inside
//! archive 1. The manifest lists X (Windows, enUS: archive 1), Y (OSX only:
//! archive 1), Z (Windows: archive 2), L (Windows: loose) and W (Windows: on
//! no CDN). The build config also names the encoding file (loose) and a VFS
//! file the CDN does not have.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use lanstash_app::fetch::{FetchOptions, Fetcher};
use lanstash_app::passthrough::Passthrough;
use lanstash_app::prefill::{PrefillOptions, Prefiller};
use lanstash_app::serve::{Cache, ConnState, RequestHandler, ServeOptions};
use lanstash_battlenet_app::builds::{BlizzardBuilds, BuildOptions};
use lanstash_battlenet_app::plans::{BlizzardPlans, PlanOptions};
use lanstash_battlenet_app::prefill::BlizzardJobs;
use lanstash_battlenet_domain::tact::PATCH_HOST;
use lanstash_domain::extent::Span;
use lanstash_domain::http::RequestHead;
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::Clock;
use lanstash_ports::events::NoEvents;
use lanstash_store::index::Lookup;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

use lanstash_app::test_support::{Capture, FakeResponse, FakeUpstream, SeenRequest};
use lanstash_battlenet_domain::profile::Blizzard;
use lanstash_domain::service::Profiles;

pub struct Tick(pub AtomicU64);
impl Clock for Tick {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

pub const CDN: &str = "eu.cdn.blizzard.com";
/// The region's second CDN host (the fake CDN serves both alike).
pub const CDN_2: &str = "level3.blizzard.com";

fn cdn_route(up: &FakeUpstream, target: &str, r: FakeResponse) {
    up.route(CDN, target, r.clone());
    up.route(CDN_2, target, r);
}
pub const BUILD: &str = "b0000000000000000000000000000001";
pub const CDN_CONFIG: &str = "c0000000000000000000000000000002";
pub const PRE_BUILD: &str = "b0000000000000000000000000000009";
pub const ARCHIVE_1: &str = "a1000000000000000000000000000003";
pub const ARCHIVE_2: &str = "a2000000000000000000000000000004";
pub const FILE_INDEX: &str = "f1000000000000000000000000000005";
pub const PATCH_ARCHIVE: &str = "9a000000000000000000000000000006";
pub const PATCH_FILE_INDEX: &str = "9f000000000000000000000000000008";
const CKEY: &str = "cc000000000000000000000000000007";

pub fn key(b: u8) -> String {
    format!("{b:02x}").repeat(16)
}
pub const DL: u8 = 0xd1;
pub const ENC: u8 = 0xe1;
pub const VFS: u8 = 0xf6;
pub const X: u8 = 0x01;
pub const Y: u8 = 0x02;
pub const Z: u8 = 0x03;
pub const L: u8 = 0x04;
pub const W: u8 = 0x05;

/// Sizes of the files.
pub const X_SIZE: u64 = 1000;
pub const Y_SIZE: u64 = 2000;
pub const Z_SIZE: u64 = 3000;
pub const L_SIZE: u64 = 400;
pub const ENC_SIZE: u64 = 300;

pub fn at(kind: &str, hash: &str, suffix: &str) -> String {
    format!("/tpr/osi/{kind}/{}/{}/{hash}{suffix}", &hash[..2], &hash[2..4])
}

/// A CDN index: entries (key, size, offset) in 4 KiB blocks, a table of contents, the footer.
pub fn index(entries: &[(u8, u64, Option<u64>)]) -> Vec<u8> {
    let offset_bytes = if entries.first().is_some_and(|e| e.2.is_some()) { 4 } else { 0 };
    let mut d = Vec::new();
    for (k, size, offset) in entries {
        d.extend_from_slice(&[*k; 16]);
        d.extend_from_slice(&(*size as u32).to_be_bytes());
        if let Some(o) = offset {
            d.extend_from_slice(&(*o as u32).to_be_bytes());
        }
    }
    d.resize(4096, 0);
    d.extend_from_slice(&[0; 24]);
    d.extend_from_slice(&[1, 0, 0, 4, offset_bytes, 4, 16, 8]);
    d.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    d.extend_from_slice(&[0; 8]);
    d
}

/// A download manifest (version 1, no checksums) in a one-chunk BLTE file.
pub fn manifest(entries: &[(u8, u64)], tags: &[(&str, u16, &[u8])]) -> Vec<u8> {
    let mut d = b"DL\x01\x10\x00".to_vec();
    d.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    d.extend_from_slice(&(tags.len() as u16).to_be_bytes());
    for (k, size) in entries {
        d.extend_from_slice(&[*k; 16]);
        d.extend_from_slice(&size.to_be_bytes()[3..]);
        d.push(0);
    }
    for (name, kind, keys) in tags {
        d.extend_from_slice(name.as_bytes());
        d.push(0);
        d.extend_from_slice(&kind.to_be_bytes());
        let mut mask = vec![0u8; entries.len().div_ceil(8)];
        for (i, (k, _)) in entries.iter().enumerate() {
            if keys.contains(k) {
                mask[i / 8] |= 0x80 >> (i % 8);
            }
        }
        d.extend_from_slice(&mask);
    }
    let mut blte = b"BLTE\0\0\0\0N".to_vec();
    blte.extend_from_slice(&d);
    blte
}

pub struct Bnet {
    pub up: Arc<FakeUpstream>,
    pub clock: Arc<Tick>,
    pub cache: Arc<Cache>,
    pub store: Arc<Store>,
    pub plans: Arc<BlizzardPlans>,
    pub builds: Arc<BlizzardBuilds>,
    pub prefill: Prefiller,
    /// Where X, Y and the manifest lie in archive 1.
    pub x_at: u64,
}

fn versions(build: &str, name: &str) -> String {
    format!(
        "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0\n\
         ## seqn = 1\neu|{build}|{CDN_CONFIG}||7|{name}\nus|{build}|{CDN_CONFIG}||7|{name}\n"
    )
}

/// The CDN above; `bgdl` lists [`PRE_BUILD`] when `pre`.
pub fn bnet(pre: bool) -> Bnet {
    bnet_with(pre, PrefillOptions::default())
}

/// [`bnet`] with prefill options.
pub fn bnet_with(pre: bool, prefill_opts: PrefillOptions) -> Bnet {
    world(pre, prefill_opts, PlanOptions::default().parallel)
}

/// [`bnet`] whose plans read their configs and indexes `parallel` at a time.
pub fn bnet_planning(parallel: usize) -> Bnet {
    world(false, PrefillOptions::default(), parallel)
}

fn world(pre: bool, prefill_opts: PrefillOptions, plan_parallel: usize) -> Bnet {
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let store = Arc::new(store);
    let up = Arc::new(FakeUpstream::default());
    let mut hosts = HostRules::default();
    hosts.add_host("blizzard", "cdn.blizzard.com");
    hosts.add_host("blizzard", CDN_2);
    let fetcher = Arc::new(Fetcher::new(up.clone(), Arc::clone(&store), Arc::new(NoEvents), FetchOptions::default()));
    let pass = Arc::new(Passthrough::new(up.clone(), 1 << 20, 4096));
    let cache = Arc::new(
        Cache::new(
            Arc::clone(&store),
            hosts,
            &Profiles::new(vec![Arc::new(Blizzard)]),
            Arc::new(NoEvents),
            ServeOptions::default(),
        )
        .with_upstream(fetcher, pass),
    );
    up.route(PATCH_HOST, "/osi/versions", FakeResponse::ok(versions(BUILD, "3.3.7").as_bytes(), &[]));
    let bgdl =
        if pre { versions(PRE_BUILD, "3.4.0") } else { "Region!STRING:0|BuildConfig!HEX:16\n## seqn = 1\n".into() };
    up.route(PATCH_HOST, "/osi/bgdl", FakeResponse::ok(bgdl.as_bytes(), &[]));
    let cdns = format!("Name!STRING:0|Path!STRING:0|Hosts!STRING:0\neu|tpr/osi|{CDN} level3.blizzard.com\n");
    up.route(PATCH_HOST, "/osi/cdns", FakeResponse::ok(cdns.as_bytes(), &[]));

    let dl = manifest(
        &[(X, X_SIZE), (Y, Y_SIZE), (Z, Z_SIZE), (L, L_SIZE), (W, 50)],
        &[
            ("Windows", 1, &[X, Z, L, W]),
            ("OSX", 1, &[Y]),
            ("enUS", 2, &[X, Y, Z, L, W]),
            ("x86_64", 3, &[X, Y, Z, L, W]),
        ],
    );
    let dl_size = dl.len() as u64;
    // Archive 1: the manifest at 0, X after a 10-byte gap, Y far behind.
    let x_at = dl_size + 10;
    let y_at = x_at + X_SIZE + 100_000;
    let mut a1 = dl.clone();
    a1.resize(x_at as usize, 0);
    a1.extend(std::iter::repeat_n(X, X_SIZE as usize));
    a1.resize(y_at as usize, 0);
    a1.extend(std::iter::repeat_n(Y, Y_SIZE as usize));
    let a2 = vec![Z; Z_SIZE as usize];

    for (name, build) in [("", BUILD), ("pre", PRE_BUILD)] {
        let bc = format!(
            "# Build Configuration {name}\nroot = {CKEY}\nencoding = {CKEY} {}\nencoding-size = 1 {ENC_SIZE}\n\
             download = {CKEY} {}\nvfs-1 = {CKEY} {}\n",
            key(ENC),
            key(DL),
            key(VFS)
        );
        cdn_route(&up, &at("config", build, ""), FakeResponse::ok(bc.as_bytes(), &[]));
    }
    let cc = format!(
        "archives = {ARCHIVE_1} {ARCHIVE_2}\nfile-index = {FILE_INDEX}\npatch-archives = {PATCH_ARCHIVE}\n\
         patch-file-index = {PATCH_FILE_INDEX}\n"
    );
    for p in [PATCH_ARCHIVE, PATCH_FILE_INDEX] {
        cdn_route(&up, &at("patch", p, ".index"), FakeResponse::ranged(&index(&[(0x77, 10, None)]), &[]));
    }
    cdn_route(&up, &at("config", CDN_CONFIG, ""), FakeResponse::ok(cc.as_bytes(), &[]));
    let i1 = index(&[(DL, dl_size, Some(0)), (X, X_SIZE, Some(x_at)), (Y, Y_SIZE, Some(y_at))]);
    cdn_route(&up, &at("data", ARCHIVE_1, ".index"), FakeResponse::ranged(&i1, &[]));
    cdn_route(&up, &at("data", ARCHIVE_2, ".index"), FakeResponse::ranged(&index(&[(Z, Z_SIZE, Some(0))]), &[]));
    let fi = index(&[(ENC, ENC_SIZE, None), (L, L_SIZE, None)]);
    cdn_route(&up, &at("data", FILE_INDEX, ".index"), FakeResponse::ranged(&fi, &[]));
    cdn_route(&up, &at("data", ARCHIVE_1, ""), FakeResponse::ranged(&a1, &[]));
    cdn_route(&up, &at("data", ARCHIVE_2, ""), FakeResponse::ranged(&a2, &[]));
    cdn_route(&up, &at("data", &key(ENC), ""), FakeResponse::ranged(&vec![ENC; ENC_SIZE as usize], &[]));
    cdn_route(&up, &at("data", &key(L), ""), FakeResponse::ranged(&vec![L; L_SIZE as usize], &[]));

    let tags = ["Windows", "x86_64", "enUS", "EU"].map(String::from).to_vec();
    let popts = PlanOptions { region: "eu".into(), tags, gap: 64, parallel: plan_parallel, ..PlanOptions::default() };
    let plans = Arc::new(BlizzardPlans::new(cache.clone(), Arc::clone(&store), up.clone(), clock.clone(), popts));
    let bopts = BuildOptions { region: "eu".into(), ..BuildOptions::default() };
    let builds = Arc::new(
        BlizzardBuilds::new(up.clone(), Arc::clone(&store), clock.clone(), bopts).with_plans(Arc::clone(&plans)),
    );
    let jobs = Arc::new(BlizzardJobs::new(Arc::clone(&builds)));
    let prefill = Prefiller::new(jobs, "blizzard", cache.clone(), Arc::clone(&store), clock.clone(), prefill_opts);
    Bnet { up, clock, cache, store, plans, builds, prefill, x_at }
}

impl Bnet {
    /// A client's GET of `target` (with `range`), waited for until stored.
    pub fn get(&self, target: &str, range: Option<&str>) {
        let mut headers = vec![("host".into(), CDN.into())];
        if let Some(r) = range {
            headers.push(("range".into(), r.into()));
        }
        let req = RequestHead { method: "GET".into(), target: target.into(), minor_version: 1, headers };
        self.cache.handle(&req, &mut ConnState::default(), &mut Capture::default()).unwrap();
        self.settle();
    }

    /// Waits for the cache's downloads to finish.
    pub fn settle(&self) {
        while self.cache.fetcher().unwrap().in_flight() > 0 {
            std::thread::yield_now();
        }
    }

    /// Whether the stash holds `span` of `target` (all of it when `None`).
    pub fn holds(&self, target: &str, span: Option<Span>) -> bool {
        match self.store.index().lookup(&format!("blizzard{target}")) {
            Lookup::Found(o) => o.covers(span.unwrap_or(Span::new(0, o.total))),
            _ => false,
        }
    }

    /// Asks the builds of `osi` until its plan is made.
    pub fn planned(&self) -> lanstash_battlenet_app::builds::BuildCheck {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let c = self.builds.check("osi");
            let done = c.build.as_ref().is_some_and(|b| b.coverage.is_some() || b.plan_error.is_some())
                && c.prerelease.as_ref().is_none_or(|b| b.coverage.is_some() || b.plan_error.is_some());
            if done || Instant::now() > until {
                self.settle();
                return self.builds.check("osi");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The paths the CDN was asked for, in order, with their ranges.
    pub fn asked(&self) -> Vec<(String, Option<String>)> {
        let seen = self.up.seen.lock().unwrap();
        let range = |r: &SeenRequest| r.headers.iter().find(|h| h.0.eq_ignore_ascii_case("range")).map(|h| h.1.clone());
        seen.iter().filter(|r| r.host == CDN || r.host == CDN_2).map(|r| (r.target.clone(), range(r))).collect()
    }
}
