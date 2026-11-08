//! A platform whose files can be checked: what the CDN sends goes straight
//! into the stash, checked as it streams, and is stored only once it passes.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_app::prefill::PieceCheck;
use lanstash_app::test_support::FakeResponse;
use lanstash_domain::extent::Span;
use lanstash_domain::object::object_key;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState};
use lanstash_store::index::Lookup;

use super::{CDN, Fixed, World, job, world};

fn holds(w: &World, target: &str, span: Span) -> bool {
    matches!(w.store.index().lookup(&object_key("blizzard", target)), Lookup::Found(o) if o.covers(span))
}

fn checked(jobs: Vec<lanstash_app::prefill::Job>) -> Fixed {
    Fixed { check_cdn: true, ..Fixed::new(jobs) }
}

#[test]
fn what_passes_its_check_is_stored_without_going_through_the_cache() {
    let w = world();
    w.up.route(CDN, "/tpr/osi/config/ab/cd/abcd", FakeResponse::ok(b"0123456789", &[]));
    w.up.route(CDN, "/tpr/osi/data/ab/cd/archive", FakeResponse::ranged(&[7u8; 4096], &[]));
    let jobs = vec![job("/tpr/osi/config/ab/cd/abcd", None), job("/tpr/osi/data/ab/cd/archive", Some((1000, 3000)))];
    let p = w.with_checked(Arc::new(checked(jobs)));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_done, s.files_rejected, s.bytes), (PrefillState::Done, 2, 0, 10 + 2000), "{s:?}");
    assert!(holds(&w, "/tpr/osi/config/ab/cd/abcd", Span::new(0, 10)));
    assert!(holds(&w, "/tpr/osi/data/ab/cd/archive", Span::new(1000, 3000)));
    assert!(!holds(&w, "/tpr/osi/data/ab/cd/archive", Span::new(0, 1000)), "only the range asked for");
    let counters = &w.cache.fetcher().unwrap().counters;
    assert_eq!(counters.stored_records.load(Ordering::Relaxed), 0, "not through the cache");
}

#[test]
fn what_fails_its_check_is_asked_once_more_then_ends_the_job_and_nothing_is_stored() {
    let w = world();
    w.up.route(CDN, "/tpr/osi/config/ab/cd/abcd", FakeResponse::ok(b"0123456789", &[]));
    let planner = checked(vec![job("/tpr/osi/config/ab/cd/abcd", None)]);
    planner.accept.store(false, Ordering::SeqCst);
    let p = w.with_checked(Arc::new(planner));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_rejected, s.bytes), (PrefillState::Failed, 1, 0), "{s:?}");
    assert!(s.error.as_deref().is_some_and(|e| e.contains("failed its check twice")), "{s:?}");
    assert_eq!(w.up.calls(), 2, "asked twice");
    assert!(!holds(&w, "/tpr/osi/config/ab/cd/abcd", Span::new(0, 1)), "nothing of it stored");
}

#[test]
fn a_file_the_cdn_lacks_is_counted_and_one_that_comes_short_is_not_stored() {
    let w = world();
    w.up.route(CDN, "/tpr/osi/gone", FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) });
    let p = w.with_checked(Arc::new(checked(vec![job("/tpr/osi/gone", None)])));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_missing), (PrefillState::Done, 1), "{s:?}");
    // A range answered with the whole file: not what was asked, never stored.
    let w = world();
    w.up.route(CDN, "/tpr/osi/data/ab/cd/archive", FakeResponse::ok(&[7u8; 4096], &[]));
    let p = w.with_checked(Arc::new(checked(vec![job("/tpr/osi/data/ab/cd/archive", Some((0, 100)))])));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!(s.state, PrefillState::Failed, "{s:?}");
    assert!(!holds(&w, "/tpr/osi/data/ab/cd/archive", Span::new(0, 1)));
}

/// A check that cannot judge what it is fed.
struct Unjudged;

impl PieceCheck for Unjudged {
    fn feed(&mut self, _data: &[u8]) {}
    fn finish(&self) -> bool {
        false
    }
    fn checks(&self) -> bool {
        false
    }
}

#[test]
fn what_its_check_cannot_judge_goes_through_the_cache_as_before() {
    use lanstash_app::prefill::{Planner, Work};
    use lanstash_ports::prefill::StartError;
    struct Blind;
    impl Planner for Blind {
        fn admit(&self, _game: &str) -> Result<(), StartError> {
            Ok(())
        }
        fn work(&self, _game: &str, _pre: bool, _version: &dyn Fn(&str)) -> Result<Work, String> {
            Ok(Work {
                hosts: vec![CDN.into()],
                jobs: vec![job("/tpr/osi/odd", None)],
                bytes: 10,
                check: Box::new(|_, _, _, _| Box::new(Unjudged)),
                parts: Box::new(|_| Vec::new()),
                check_cdn: true,
            })
        }
    }
    let w = world();
    w.up.route(CDN, "/tpr/osi/odd", FakeResponse::ok(b"0123456789", &[]));
    let p = w.with_checked(Arc::new(Blind));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_rejected), (PrefillState::Done, 0), "{s:?}");
    assert!(holds(&w, "/tpr/osi/odd", Span::new(0, 10)));
    assert_eq!(w.cache.fetcher().unwrap().counters.stored_records.load(Ordering::Relaxed), 1, "through the cache");
}
