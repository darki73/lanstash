//! Prefill from lancache: each request is read out of lancache's cache and
//! checked against the hash in its path before it is stored; what lancache
//! does not hold, or what fails the check, comes from the CDN. The fixture's
//! hashes are made up, so nothing of it can pass the check: these tests show
//! the safe path (the components and real files show the other).

use std::sync::Arc;

use lanstash_app::lancache_read::{LancacheOptions, LancacheRead};
use lanstash_app::stash_put::StashPut;
use lanstash_battlenet_domain::profile::Blizzard;
use lanstash_battlenet_domain::tact::Version;
use lanstash_domain::extent::Span;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState, StartError};
use lanstash_store::test_support::nginx::MemNginx;

use super::support::*;
use super::wait;

const MIB: u64 = 1 << 20;

/// The fixture with lancache holding `copy` of the install's files (what the CDN has).
fn with_lancache(copy: bool) -> (Bnet, Arc<MemNginx>) {
    let mut w = bnet(false);
    let nginx = Arc::new(MemNginx::default());
    if copy {
        let v = Version { build_config: BUILD.into(), cdn_config: CDN_CONFIG.into(), name: "3.3.7".into() };
        let plan = w.plans.plan("osi", &v).unwrap();
        for t in plan.files.iter().map(|f| &f.0).chain(&plan.archives) {
            if let Some(body) = w.up.body(CDN, t) {
                nginx.whole(&format!("blizzard{t}"), &body, MIB);
            }
        }
    }
    let read = LancacheRead::new(
        Arc::clone(&nginx) as _,
        LancacheOptions { service: "blizzard".into(), slice_bytes: MIB, max_piece: 64 * MIB },
    );
    let put = StashPut::new(Arc::clone(&w.store), Arc::new(Blizzard), 16 * MIB);
    w.prefill = w.prefill.with_lancache(read, put);
    (w, nginx)
}

#[test]
fn what_fails_its_check_comes_from_the_cdn() {
    let (w, nginx) = with_lancache(true);
    assert!(w.prefill.lancache());
    w.prefill.start("osi", false, PrefillFrom::Lancache).unwrap();
    let s = wait(&w);
    assert_eq!((s.state, s.from, s.error.as_deref()), (PrefillState::Done, PrefillFrom::Lancache, None), "{s:?}");
    // Planning stashed nothing (the stash held none of the game): all 11 were looked for in lancache.
    assert_eq!((s.files_done, s.files_skipped, s.files_local, s.bytes_local), (11, 0, 0, 0), "{s:?}");
    assert_eq!(s.files_rejected, 11, "made-up hashes never pass: {s:?}");
    assert!(!nginx.opened.lock().unwrap().is_empty(), "lancache was asked");
    // The install is stashed all the same, from the CDN.
    assert!(w.holds(&at("data", ARCHIVE_1, ""), Some(Span::new(w.x_at, w.x_at + X_SIZE))));
    assert!(w.holds(&at("data", ARCHIVE_2, ""), None));
    assert!(w.holds(&at("data", &key(L), ""), None) && w.holds(&at("data", &key(ENC), ""), None));
}

#[test]
fn what_lancache_lacks_comes_from_the_cdn() {
    let (w, _) = with_lancache(false);
    w.prefill.start("osi", false, PrefillFrom::Lancache).unwrap();
    let s = wait(&w);
    assert_eq!((s.state, s.files_done, s.files_local, s.files_rejected), (PrefillState::Done, 11, 0, 0), "{s:?}");
    assert!(w.holds(&at("data", ARCHIVE_2, ""), None));
}

#[test]
fn from_lancache_needs_one() {
    let w = bnet(false);
    assert!(!w.prefill.lancache());
    assert_eq!(w.prefill.start("osi", false, PrefillFrom::Lancache), Err(StartError::NoLancache));
    assert!(w.prefill.status().is_none(), "nothing started");
    // From the CDN, the source is said.
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    assert_eq!(wait(&w).from, PrefillFrom::Cdn);
}
