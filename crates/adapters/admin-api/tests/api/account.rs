//! Tests for `lanstash_adapter_admin_api::api::account`: a platform's account
//! and its QR sign-in.

use lanstash_ports::account::Account;

use super::*;

#[derive(Default)]
struct FakeAccount {
    waiting: Mutex<bool>,
    calls: Mutex<Vec<String>>,
}

impl Account for FakeAccount {
    fn view(&self) -> String {
        format!("{{\"waiting\":{}}}\n", self.waiting.lock().unwrap())
    }
    fn start_sign_in(&self) -> Result<(), String> {
        *self.waiting.lock().unwrap() = true;
        self.calls.lock().unwrap().push("start".into());
        Ok(())
    }
    fn cancel_sign_in(&self) {
        *self.waiting.lock().unwrap() = false;
        self.calls.lock().unwrap().push("cancel".into());
    }
    fn sign_out(&self, account: &str) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("out {account}"));
        Err("read-only file system".into())
    }
    fn sign_in_with_code(&self, pasted: &str) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("code {pasted}"));
        if pasted == "good" { Ok(()) } else { Err("Epic: that code was used".into()) }
    }
    fn set_client(&self, client: Option<&str>) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("client {client:?}"));
        if client == Some("bad") { Err("a client is <id>:<secret>".into()) } else { Ok(()) }
    }
    fn qr_svg(&self) -> Option<String> {
        self.waiting.lock().unwrap().then(|| "<svg/>".to_string())
    }
}

#[derive(Debug)]
struct Steamy(Arc<FakeAccount>);

impl std::fmt::Debug for FakeAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FakeAccount")
    }
}

impl Platform for Steamy {
    fn id(&self) -> &str {
        "steam"
    }
    fn account(&self) -> Option<Arc<dyn Account>> {
        Some(Arc::clone(&self.0) as Arc<dyn Account>)
    }
}

#[test]
fn the_account_is_shown_signed_in_to_and_its_code_served() {
    let acc = Arc::new(FakeAccount::default());
    let a = api().with_platforms(vec![
        Arc::new(Steamy(Arc::clone(&acc))),
        Arc::new(Echo { id: "blizzard", views: true, jobs: None }),
    ]);
    assert_eq!(get(&a, "GET", "/api/v1/account/steam").body, "{\"waiting\":false}\n");
    assert_eq!(get(&a, "GET", "/api/v1/account/steam/qr.svg").status, 404, "no sign-in waits");
    let s = get(&a, "POST", "/api/v1/account/steam/sign-in");
    assert_eq!((s.status, s.body.as_str()), (202, "{\"waiting\":true}\n"));
    let q = get(&a, "GET", "/api/v1/account/steam/qr.svg");
    assert_eq!((q.status, q.body.as_str()), (200, "<svg/>"));
    assert!(q.headers.contains(&("Content-Type".into(), "image/svg+xml".into())));
    assert_eq!(get(&a, "DELETE", "/api/v1/account/steam/sign-in").body, "{\"waiting\":false}\n");
    let none = get(&a, "POST", "/api/v1/account/steam/sign-out");
    assert_eq!(none.status, 400, "which account");
    let out = get(&a, "POST", "/api/v1/account/steam/sign-out?account=player_one");
    assert_eq!((out.status, out.body.as_str()), (500, "{\"error\":\"read-only file system\",\"kind\":\"internal\"}\n"));
    assert_eq!(*acc.calls.lock().unwrap(), ["start", "cancel", "out player_one"]);
    let ok = get(&a, "POST", "/api/v1/account/steam/sign-in?code=good");
    assert_eq!(ok.status, 200, "a pasted code signs in at once");
    let bad = get(&a, "POST", "/api/v1/account/steam/sign-in?code=%7B%22authorizationCode%22%7D");
    assert_eq!(
        (bad.status, bad.body.as_str()),
        (400, "{\"error\":\"Epic: that code was used\",\"kind\":\"refused\"}\n")
    );
    assert_eq!(acc.calls.lock().unwrap()[3..], ["code good", "code {\"authorizationCode\"}"], "decoded");
    assert_eq!(get(&a, "POST", "/api/v1/account/steam/client?value=ab%3Acd").status, 200);
    assert_eq!(get(&a, "POST", "/api/v1/account/steam/client?value=bad").status, 400);
    assert_eq!(get(&a, "POST", "/api/v1/account/steam/client").status, 400, "which client");
    assert_eq!(get(&a, "DELETE", "/api/v1/account/steam/client").status, 200);
    assert_eq!(acc.calls.lock().unwrap()[5..], ["client Some(\"ab:cd\")", "client Some(\"bad\")", "client None"]);
    // In the header, not the path: what logs and histories keep never holds it.
    let with = |target: &str, secret: &str| {
        let headers = vec![("x-lanstash-secret".into(), secret.into())];
        let req = RequestHead { method: "POST".into(), target: target.into(), minor_version: 1, headers };
        let mut c = Capture::default();
        a.handle(&req, &mut ConnState::default(), &mut c).unwrap();
        c.status
    };
    assert_eq!(with("/api/v1/account/steam/sign-in", "good"), 200);
    assert_eq!(with("/api/v1/account/steam/client", "id%3Asecret"), 200);
    assert_eq!(acc.calls.lock().unwrap()[8..], ["code good", "client Some(\"id:secret\")"], "decoded");
}

#[test]
fn platforms_without_an_account_and_unknown_paths_are_404() {
    let a = api().with_platforms(vec![
        Arc::new(Steamy(Arc::new(FakeAccount::default()))),
        Arc::new(Echo { id: "blizzard", views: true, jobs: None }),
    ]);
    for (method, target) in [
        ("GET", "/api/v1/account/blizzard"),
        ("GET", "/api/v1/account/nope"),
        ("GET", "/api/v1/account/steam/other"),
        ("POST", "/api/v1/account/steam"),
        ("DELETE", "/api/v1/account/steam/sign-out"),
    ] {
        assert_eq!(get(&a, method, target).status, 404, "{method} {target}");
    }
    assert_eq!(get(&a, "PUT", "/api/v1/account/steam").status, 405);
}
