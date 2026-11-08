//! Tests for `lanstash_app::auto_update`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_app::auto_update::{AutoUpdate, KEPT};
use lanstash_domain::window::Window;
use lanstash_ports::clock::Clock;
use lanstash_ports::keep::Keep;
use lanstash_ports::platform::{BuildStatus, Platform};
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillStatus, StartError};

const HOUR: u64 = 3_600_000;

struct Tick(AtomicU64);

impl Clock for Tick {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Default)]
struct Jobs(Mutex<Vec<(String, PrefillFrom)>>);

impl Prefill for Jobs {
    fn start(&self, game: &str, _pre: bool, from: PrefillFrom) -> Result<(), StartError> {
        self.0.lock().unwrap().push((game.into(), from));
        Ok(())
    }
    fn queue(&self, game: &str, pre: bool, from: PrefillFrom) -> Result<usize, StartError> {
        self.start(game, pre, from).map(|()| 0)
    }
    fn stop(&self) {}
    fn status(&self) -> Option<PrefillStatus> {
        None
    }
}

#[derive(Debug)]
struct Shelf {
    builds: Mutex<Vec<BuildStatus>>,
    jobs: Arc<Jobs>,
}

impl Platform for Shelf {
    fn id(&self) -> &str {
        "steam"
    }
    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        Some(self.builds.lock().unwrap().clone())
    }
    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        Some(Arc::clone(&self.jobs) as Arc<dyn Prefill>)
    }
}

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
}

fn build(game: &str, version: &str, known: bool, current: bool) -> BuildStatus {
    BuildStatus { game: game.into(), version: version.into(), known, current }
}

fn world(builds: Vec<BuildStatus>) -> (Arc<Shelf>, Arc<Tick>) {
    let shelf = Arc::new(Shelf { builds: Mutex::new(builds), jobs: Arc::default() });
    (shelf, Arc::new(Tick(AtomicU64::new(12 * HOUR))))
}

fn queued(shelf: &Shelf) -> Vec<String> {
    shelf.jobs.0.lock().unwrap().iter().map(|(g, _)| g.clone()).collect()
}

#[test]
fn a_game_once_held_whole_is_updated_in_the_window_once_per_build() {
    let (shelf, clock) = world(vec![build("10", "1", true, true), build("20", "1", true, false)]);
    let window = Window::parse("00:00-04:00").unwrap();
    let auto = AutoUpdate::new(vec![shelf.clone() as Arc<dyn Platform>], clock.clone(), window);
    assert!(auto.step().is_empty());
    assert_eq!(auto.followed(), ["steam/10"], "the partly held game is not followed");
    // A new build of 10, outside the window: nothing yet.
    *shelf.builds.lock().unwrap() = vec![build("10", "2", true, false), build("20", "1", true, false)];
    assert!(auto.step().is_empty());
    // In the window: queued from the CDN, once.
    clock.0.store(25 * HOUR, Ordering::SeqCst);
    assert_eq!(auto.step(), [("steam".to_owned(), "10".to_owned())]);
    assert_eq!(*shelf.jobs.0.lock().unwrap(), [("10".to_owned(), PrefillFrom::Cdn)]);
    assert!(auto.step().is_empty(), "not again for the same build");
    // Its next build is queued again; one not worked out yet waits.
    *shelf.builds.lock().unwrap() = vec![build("10", "3", false, false)];
    assert!(auto.step().is_empty(), "not known yet");
    *shelf.builds.lock().unwrap() = vec![build("10", "3", true, false)];
    assert_eq!(auto.step().len(), 1);
    assert_eq!(queued(&shelf), ["10", "10"]);
}

#[test]
fn a_game_deleted_whole_is_no_longer_followed() {
    use lanstash_app::delete_game::GameDeleter;
    use lanstash_ports::events::NoEvents;
    use lanstash_store::store::{Roots, Store, StoreOptions};
    use lanstash_store::test_support::MemFs;
    // Held whole, deleted by hand, then seen part-held again: that is not a new build.
    let (shelf, clock) = world(vec![build("10", "1", true, true)]);
    clock.0.store(25 * HOUR, Ordering::SeqCst);
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let deleter = Arc::new(GameDeleter::new(Arc::new(store), Arc::new(NoEvents), clock.clone()));
    let window = Window::parse("00:00-04:00").unwrap();
    let auto = AutoUpdate::new(vec![shelf.clone() as Arc<dyn Platform>], clock, window).with_deleter(deleter.clone());
    assert!(auto.step().is_empty());
    assert_eq!(auto.followed(), ["steam/10"]);
    deleter.start("steam", "10").unwrap();
    *shelf.builds.lock().unwrap() = vec![build("10", "1", true, false)];
    assert!(auto.step().is_empty(), "not fetched back");
    assert!(auto.followed().is_empty());
    assert!(deleter.take_deleted().is_empty(), "taken by the pass");
}

fn deleting_world() -> (Arc<Shelf>, AutoUpdate, Arc<lanstash_app::delete_game::GameDeleter>) {
    use lanstash_app::delete_game::GameDeleter;
    use lanstash_ports::events::NoEvents;
    use lanstash_store::store::{Roots, Store, StoreOptions};
    use lanstash_store::test_support::MemFs;
    let (shelf, clock) = world(vec![build("10", "1", true, true)]);
    clock.0.store(25 * HOUR, Ordering::SeqCst);
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let deleter = Arc::new(GameDeleter::new(Arc::new(store), Arc::new(NoEvents), clock.clone()));
    let window = Window::parse("00:00-04:00").unwrap();
    let auto = AutoUpdate::new(vec![shelf.clone() as Arc<dyn Platform>], clock, window).with_deleter(deleter.clone());
    assert!(auto.step().is_empty());
    (shelf, auto, deleter)
}

#[test]
fn a_status_from_before_the_deletion_does_not_follow_the_game_again() {
    let (shelf, auto, deleter) = deleting_world();
    deleter.start("steam", "10").unwrap();
    assert!(auto.step().is_empty());
    assert!(auto.followed().is_empty(), "the status still says current, but the game was deleted");
    // Later a client fetches part of it: known, not current, and not a new build.
    *shelf.builds.lock().unwrap() = vec![build("10", "1", true, false)];
    assert!(auto.step().is_empty());
    *shelf.builds.lock().unwrap() = vec![build("10", "2", true, false)];
    assert!(auto.step().is_empty(), "a deleted game is not fetched back");
}

#[test]
fn a_deleted_game_held_whole_again_is_followed_again() {
    let (shelf, auto, deleter) = deleting_world();
    deleter.start("steam", "10").unwrap();
    auto.step();
    // Gone from the status, then downloaded whole again by hand.
    *shelf.builds.lock().unwrap() = vec![];
    auto.step();
    *shelf.builds.lock().unwrap() = vec![build("10", "1", true, true)];
    auto.step();
    assert_eq!(auto.followed(), ["steam/10"]);
}

#[derive(Debug)]
struct Busy;

impl Prefill for Busy {
    fn start(&self, _game: &str, _pre: bool, _from: PrefillFrom) -> Result<(), StartError> {
        Err(StartError::Busy)
    }
    fn stop(&self) {}
    fn status(&self) -> Option<PrefillStatus> {
        None
    }
}

#[derive(Debug)]
struct BusyShelf(Mutex<Vec<BuildStatus>>);

impl Platform for BusyShelf {
    fn id(&self) -> &str {
        "steam"
    }
    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        Some(self.0.lock().unwrap().clone())
    }
    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        Some(Arc::new(Busy))
    }
}

#[test]
fn a_build_that_could_not_be_queued_is_said_with_why_and_not_tried_again() {
    use lanstash_app::auto_update::Pass;
    let shelf = Arc::new(BusyShelf(Mutex::new(vec![build("10", "1", true, true)])));
    let clock = Arc::new(Tick(AtomicU64::new(12 * HOUR)));
    let window = Window::parse("00:00-24:00").unwrap();
    let auto = AutoUpdate::new(vec![shelf.clone() as Arc<dyn Platform>], clock, window);
    assert_eq!(auto.pass(), Pass::default());
    *shelf.0.lock().unwrap() = vec![build("10", "2", true, false)];
    let pass = auto.pass();
    assert!(pass.queued.is_empty());
    assert_eq!(pass.refused, [("steam".to_owned(), "10".to_owned(), "Busy".to_owned())]);
    assert_eq!(auto.pass(), Pass::default(), "not tried again for the same build");
}

#[test]
fn the_followed_games_are_kept_across_restarts() {
    let keep: Arc<Memory> = Arc::default();
    let (shelf, clock) = world(vec![build("10", "1", true, true)]);
    let window = Window::parse("00:00-24:00").unwrap();
    let auto = AutoUpdate::new(vec![shelf.clone() as Arc<dyn Platform>], clock.clone(), window)
        .with_keep(Arc::clone(&keep) as Arc<dyn Keep>);
    auto.step();
    assert_eq!(keep.load(KEPT).unwrap().unwrap(), b"steam/10\n");
    // After a restart, the new build is behind from the first look: still followed.
    *shelf.builds.lock().unwrap() = vec![build("10", "2", true, false)];
    let again =
        AutoUpdate::new(vec![shelf.clone() as Arc<dyn Platform>], clock, window).with_keep(keep as Arc<dyn Keep>);
    assert_eq!(again.step(), [("steam".to_owned(), "10".to_owned())]);
}
