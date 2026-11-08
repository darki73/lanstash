//! Tests for `api::events`: the status stream ends once its viewer may no longer see as much, and
//! one address holds only a few.

use std::net::{IpAddr, Ipv4Addr};

use lanstash_adapter_admin_api::api::events::{PER_ADDRESS, RECHECK_EVENTS};

use super::access::{accounts_on, cookie_of, send};
use super::*;

/// A page following the stream: it signs out after its first event, and counts the events.
struct SignsOut<'a> {
    api: &'a AdminApi,
    cookie: String,
    events: usize,
}

impl ResponseSink for SignsOut<'_> {
    fn send_head(&mut self, _: &ResponseHead<'_>) -> Result<(), SinkError> {
        Ok(())
    }
    fn send_stream_head(&mut self, _: u16, _: &[(&str, &str)]) -> Result<(), SinkError> {
        Ok(())
    }
    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        if data.starts_with(b"data: ") {
            self.events += 1;
            if self.events == 1 {
                send(self.api, "DELETE", "/api/v1/session", &[("cookie", &self.cookie)], false);
            }
            if self.events > 3 * RECHECK_EVENTS {
                return Err(SinkError("the stream went on".into()));
            }
        }
        Ok(())
    }
}

/// A page that holds its stream until let go.
struct Holds(Arc<(Mutex<bool>, std::sync::Condvar)>, Arc<std::sync::atomic::AtomicUsize>);

impl ResponseSink for Holds {
    fn send_head(&mut self, _: &ResponseHead<'_>) -> Result<(), SinkError> {
        Ok(())
    }
    fn send_stream_head(&mut self, _: u16, _: &[(&str, &str)]) -> Result<(), SinkError> {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    fn send_body(&mut self, _: &[u8]) -> Result<(), SinkError> {
        let (gone, cv) = &*self.0;
        drop(cv.wait_while(gone.lock().unwrap(), |g| !*g).unwrap());
        Err(SinkError("closed".into()))
    }
}

#[test]
fn one_address_holds_only_a_few_streams_and_another_still_gets_one() {
    let a = Arc::new(api());
    let gone = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let open = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let req = RequestHead { method: "GET".into(), target: "/api/v1/events".into(), minor_version: 1, headers: vec![] };
    let at = |last: u8| ConnState::new(false).with_client(IpAddr::V4(Ipv4Addr::new(192, 0, 2, last)));
    let pages: Vec<_> = (0..PER_ADDRESS)
        .map(|_| {
            let (a, gone, open, req) = (Arc::clone(&a), Arc::clone(&gone), Arc::clone(&open), req.clone());
            std::thread::spawn(move || {
                let _ = a.handle(&req, &mut at(7), &mut Holds(gone, open));
            })
        })
        .collect();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while open.load(std::sync::atomic::Ordering::SeqCst) < PER_ADDRESS {
        assert!(std::time::Instant::now() < until, "the pages' streams did not open");
        std::thread::yield_now();
    }
    let mut c = Capture::default();
    a.handle(&req, &mut at(7), &mut c).unwrap();
    assert_eq!(c.status, 503, "one stream too many from that address");
    let mut other = Holds(Arc::new((Mutex::new(true), std::sync::Condvar::new())), Arc::clone(&open));
    // It opens, then leaves at once (its page is gone already).
    let _ = a.handle(&req, &mut at(8), &mut other);
    assert_eq!(open.load(std::sync::atomic::Ordering::SeqCst), PER_ADDRESS + 1, "another address still gets one");
    *gone.0.lock().unwrap() = true;
    gone.1.notify_all();
    for p in pages {
        p.join().unwrap();
    }
}

#[test]
fn a_stream_ends_once_its_viewer_signed_out() {
    let a = accounts_on(AdminApi::new(status(), Duration::from_millis(100)));
    let cookie = cookie_of(&a, "boss");
    let req = RequestHead {
        method: "GET".into(),
        target: "/api/v1/events".into(),
        minor_version: 1,
        headers: vec![("cookie".into(), cookie.clone())],
    };
    let mut page = SignsOut { api: &a, cookie, events: 0 };
    let mut conn = ConnState::new(false).with_client(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7)));
    a.handle(&req, &mut conn, &mut page).unwrap();
    assert!(page.events < RECHECK_EVENTS, "ended at the first look after the sign-out: {} events", page.events);
}
