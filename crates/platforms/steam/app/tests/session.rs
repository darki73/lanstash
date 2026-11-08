//! Tests for `lanstash_steam_app::session`.

mod support;

use support::account::{STEAMID, services};

#[test]
fn a_session_logs_on_with_the_kept_sign_in() {
    let (s, _) = services(true);
    assert!(s.signed_in());
    let (_, on) = s.session("someone").unwrap();
    assert_eq!((on.account.as_str(), on.steamid, on.cell_id), ("someone", STEAMID, 127));
}

#[test]
fn without_a_sign_in_there_is_no_session() {
    let (s, _) = services(false);
    assert!(!s.signed_in());
    assert_eq!(s.session("someone").unwrap_err(), "someone is not signed in to Steam: sign in from the web UI");
    assert_eq!(s.with_session(|_, _| Ok(())), Err("not signed in to Steam: sign in from the web UI".to_string()));
}

/// Counts how many connections were opened.
struct Counting(std::sync::atomic::AtomicUsize);

impl lanstash_ports::link::Dialer for Counting {
    fn dial(&self, h: &str, p: u16, path: &str) -> Result<Box<dyn lanstash_ports::link::Link>, String> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        support::account::AccountDialer(false).dial(h, p, path)
    }
    fn get(&self, h: &str, path: &str) -> Result<Vec<u8>, String> {
        support::account::AccountDialer(false).get(h, path)
    }
}

#[test]
fn one_shared_session_serves_every_use_and_is_opened_again_only_when_it_broke() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (base, _) = services(true);
    let dials = Arc::new(Counting(AtomicUsize::new(0)));
    let s = lanstash_steam_app::session::Services::new(
        dials.clone(),
        base.secrets.clone(),
        base.upstream.clone(),
        Arc::new(lanstash_ports::clock::FixedClock(0)),
    );
    assert_eq!(s.with_session(|_, on| Ok(on.cell_id)), Ok(127));
    assert_eq!(s.with_session(|_, on| Ok(on.steamid)), Ok(STEAMID));
    assert_eq!(dials.0.load(Ordering::SeqCst), 1, "one logon for both");
    // Steam refusing the work (no manifest, say) keeps the session: no new logon.
    assert_eq!(s.with_session(|_, _| Err::<(), _>("no manifest".to_string())), Err("no manifest".to_string()));
    assert_eq!(dials.0.load(Ordering::SeqCst), 1, "kept");
    // A connection that broke (no answer) is opened again and the work tried once more.
    let tries = AtomicUsize::new(0);
    let once = s.with_session(|conn, _| {
        if tries.fetch_add(1, Ordering::SeqCst) == 0 {
            conn.receive().map(|_| 0) // the scripted Steam says nothing: it times out
        } else {
            Ok(7)
        }
    });
    assert_eq!((once, dials.0.load(Ordering::SeqCst)), (Ok(7), 2), "retried on a new session");
    s.forget_session("someone");
    assert_eq!(s.with_session(|_, _| Ok(())), Ok(()));
    assert_eq!(dials.0.load(Ordering::SeqCst), 3);
}

#[test]
fn a_refused_token_rests_until_a_new_sign_in_or_half_an_hour() {
    use lanstash_steam_app::login::token_name;
    use lanstash_steam_app::session::REFUSED;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (base, _) = services(true);
    let dials = Arc::new(Refusing(AtomicUsize::new(0), 5));
    struct Tick(std::sync::atomic::AtomicU64);
    impl lanstash_ports::clock::Clock for Tick {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }
    let clock = Arc::new(Tick(std::sync::atomic::AtomicU64::new(0)));
    let s = lanstash_steam_app::session::Services::new(
        dials.clone(),
        base.secrets.clone(),
        base.upstream.clone(),
        clock.clone(),
    );
    assert!(!s.token_refused("someone"));
    assert_eq!(s.with_session(|_, _| Ok(())), Err(REFUSED.to_string()));
    assert!(s.token_refused("someone"));
    assert_eq!(s.with_session(|_, _| Ok(())), Err(REFUSED.to_string()));
    assert_eq!(dials.0.load(Ordering::SeqCst), 1, "asked Steam once");
    // After a rest it is tried again (Steam may have refused it for a while only).
    clock.0.store(lanstash_steam_app::session::RETRY_REFUSED_MS, Ordering::SeqCst);
    assert_eq!(s.with_session(|_, _| Ok(())), Err(REFUSED.to_string()));
    assert_eq!(dials.0.load(Ordering::SeqCst), 2, "tried once more after the rest");
    base.secrets.save(&token_name("someone"), "NEW").unwrap();
    assert!(!s.token_refused("someone"), "a new sign-in is tried");
}

#[test]
fn a_session_in_use_is_skipped_by_the_heartbeat_not_waited_for() {
    let (s, _) = services(true);
    // From inside the work on the session: waiting for it would never end.
    assert_eq!(s.with_session(|_, _| Ok(s.beat())), Ok(true));
    assert!(s.beat(), "once free, it gets its heartbeat");
}

/// A Steam that answers every logon with `.1`, counting the connections.
struct Refusing(std::sync::atomic::AtomicUsize, u64);

impl lanstash_ports::link::Dialer for Refusing {
    fn dial(&self, _: &str, _: u16, _: &str) -> Result<Box<dyn lanstash_ports::link::Link>, String> {
        use lanstash_steam_domain::cm::message::{Header, Message, emsg};
        use lanstash_steam_domain::cm::proto::Writer;
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let code = self.1;
        let (link, _) = support::steam(Box::new(move |m| {
            if m.emsg != emsg::CLIENT_LOGON {
                return vec![];
            }
            let body = Writer::new().varint(1, code).finish();
            vec![Message::new(emsg::CLIENT_LOG_ON_RESPONSE, Header::default(), body).encode()]
        }));
        Ok(link)
    }
    fn get(&self, h: &str, path: &str) -> Result<Vec<u8>, String> {
        support::account::AccountDialer(false).get(h, path)
    }
}

#[test]
fn a_passing_logon_failure_does_not_rest_the_token() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (base, _) = services(true);
    let dials = Arc::new(Refusing(AtomicUsize::new(0), 48));
    let clock = Arc::new(lanstash_ports::clock::FixedClock(0));
    let s =
        lanstash_steam_app::session::Services::new(dials.clone(), base.secrets.clone(), base.upstream.clone(), clock);
    let e = s.with_session(|_, _| Ok(())).unwrap_err();
    assert!(e.starts_with("Steam could not log on now"), "{e}");
    assert!(!s.token_refused("someone"), "not to be signed in again");
    let _ = s.with_session(|_, _| Ok(()));
    assert_eq!(dials.0.load(Ordering::SeqCst), 2, "tried again at the next use");
}

#[test]
fn a_heartbeat_goes_out_on_the_open_session_and_logons_are_counted() {
    let (s, _) = services(true);
    assert!(!s.beat(), "no session open: nothing to keep alive");
    assert_eq!(s.logons(), 0);
    s.with_session(|_, _| Ok(())).unwrap();
    assert_eq!(s.logons(), 1);
    assert!(s.beat());
    s.with_session(|_, _| Ok(())).unwrap();
    assert_eq!(s.logons(), 1, "the kept-alive session is used again");
}
