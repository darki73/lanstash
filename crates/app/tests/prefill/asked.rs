//! A job asked for another install: refused by a platform that prefills only its own, else run
//! with the install named in its status and handed to the planner.

use super::*;

/// A planner that prefills other installs, and remembers which it was asked for.
#[derive(Debug, Default)]
struct Installs {
    asked: Mutex<Vec<Option<String>>>,
}

impl Planner for Installs {
    fn admit(&self, _game: &str) -> Result<(), StartError> {
        Ok(())
    }
    fn work(&self, game: &str, prerelease: bool, version: &dyn Fn(&str)) -> Result<Work, String> {
        self.work_for(game, prerelease, None, version)
    }
    fn installs(&self) -> bool {
        true
    }
    fn work_for(&self, _: &str, _: bool, install: Option<&str>, _: &dyn Fn(&str)) -> Result<Work, String> {
        self.asked.lock().unwrap().push(install.map(String::from));
        Ok(Work {
            hosts: vec![CDN.into()],
            jobs: Vec::new(),
            bytes: 0,
            check: Box::new(|_, _, _, _| Box::new(Verdict(true))),
            parts: Box::new(|_| Vec::new()),
            check_cdn: false,
        })
    }
}

#[test]
fn a_platform_that_prefills_only_its_own_install_refuses_another() {
    let w = world();
    let p = w.prefiller(Arc::new(Fixed::new(Vec::new())));
    assert_eq!(p.queue_for("osi", false, PrefillFrom::Cdn, Some("OSX")), Err(StartError::NoInstall));
    assert!(p.status().is_none(), "nothing started");
}

#[test]
fn another_install_is_named_in_the_status_and_handed_to_the_planner() {
    let w = world();
    let planner = Arc::new(Installs::default());
    let p = w.prefiller(planner.clone());
    assert_eq!(p.queue_for("osi", false, PrefillFrom::Cdn, Some("OSX,arm64")), Ok(0));
    let s = w.wait(&p);
    assert_eq!((s.state, s.install.as_deref()), (PrefillState::Done, Some("OSX,arm64")));
    assert_eq!(*planner.asked.lock().unwrap(), [Some("OSX,arm64".to_string())]);
    assert_eq!(p.queue_for("osi", false, PrefillFrom::Cdn, None), Ok(0), "the usual install too");
    w.wait(&p);
    assert_eq!(planner.asked.lock().unwrap().last(), Some(&None));
}
