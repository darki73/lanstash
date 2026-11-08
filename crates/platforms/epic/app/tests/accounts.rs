//! Tests for `lanstash_epic_app::accounts`, against a scripted Epic.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_epic_app::accounts::{ACCOUNTS, CLIENT, EpicAccounts, Wiring, token_name};
use lanstash_ports::account::Account;
use lanstash_ports::clock::Clock;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};

const ONE: &str = "11111111111111111111111111111111";
const TWO: &str = "22222222222222222222222222222222";

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, String>>);

impl Secrets for Memory {
    fn load(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), value.into());
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

struct Now(AtomicU64);

impl Clock for Now {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Answers token requests: a code `goodN` for account N, a refresh token `rN-k` with `rN-(k+1)`.
#[derive(Default)]
struct Epic {
    bodies: Mutex<Vec<String>>,
    /// The client each request named (its Authorization header).
    clients: Mutex<Vec<String>>,
}

fn tokens(id: &str, name: &str, n: u32) -> String {
    format!(
        r#"{{"access_token":"a{n}","expires_in":28800,"refresh_token":"r{id}-{n}","refresh_expires":1987200,"account_id":"{id}","displayName":"{name}"}}"#
    )
}

impl Web for Epic {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        assert_eq!(
            (req.method, req.host, req.target),
            ("POST", "account-public-service-prod03.ol.epicgames.com", "/account/api/oauth/token")
        );
        let auth = req.headers.iter().find(|(n, _)| *n == "Authorization").map(|(_, v)| (*v).to_string());
        self.clients.lock().unwrap().push(auth.expect("a client is named"));
        assert!(req.tls.is_some(), "over HTTPS");
        let body = String::from_utf8(req.body.to_vec()).unwrap();
        self.bodies.lock().unwrap().push(body.clone());
        let answer = |status: u16, text: String| Ok(WebResponse { status, body: text.into_bytes() });
        if body.contains("code=good1&") {
            return answer(200, tokens(ONE, "Player One", 1));
        }
        if body.contains("code=good2&") {
            return answer(200, tokens(TWO, "Player Two", 1));
        }
        for id in [ONE, TWO] {
            for k in 1..9 {
                if body.contains(&format!("refresh_token=r{id}-{k}&")) {
                    return answer(200, tokens(id, "Renewed", k + 1));
                }
            }
        }
        answer(400, r#"{"errorCode":"errors.com.epicgames.account.oauth.invalid_grant","errorMessage":"Sorry, that did not work."}"#.into())
    }
}

fn accounts() -> (EpicAccounts, Arc<Memory>, Arc<Epic>, Arc<Now>) {
    let (secrets, web, now) =
        (Arc::new(Memory::default()), Arc::new(Epic::default()), Arc::new(Now(AtomicU64::new(1_000))));
    let wiring = Wiring {
        web: web.clone(),
        secrets: secrets.clone(),
        clock: now.clone(),
        client_id: "id".into(),
        client_secret: "secret".into(),
    };
    (EpicAccounts::new(wiring), secrets, web, now)
}

#[test]
fn a_pasted_code_adds_its_account_and_another_one_joins_it() {
    let (a, secrets, _, _) = accounts();
    assert_eq!(a.add(r#"{"authorizationCode":"good1"}"#).unwrap(), "Player One");
    a.sign_in_with_code("good2").unwrap();
    assert_eq!(secrets.load(ACCOUNTS).unwrap().as_deref(), Some(format!("{ONE}\n{TWO}\n").as_str()));
    let kept = a.accounts();
    assert_eq!(kept.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["Player One", "Player Two"]);
    assert_eq!(kept[0].refresh_until_ms, 1_000 + 1_987_200_000);
    assert!(secrets.load(&token_name(ONE)).unwrap().unwrap().starts_with(&format!("r{ONE}-1\n")));
    let v = a.view();
    assert!(v.starts_with(&format!(
        "{{\"accounts\":[{{\"name\":\"Player One\",\"id\":\"{ONE}\",\"refused\":false}},{{\"name\":\"Player Two\",\"id\":\"{TWO}\",\"refused\":false}}],\"account\":\"Player One\",\"sign_in\":{{\"state\":\"idle\""
    )), "{v}");
    assert!(v.contains("\"method\":\"code\",\"login_url\":\"https://www.epicgames.com/id/login?redirectUrl="), "{v}");
}

#[test]
fn a_refused_or_wrong_code_says_why_and_adds_nothing() {
    let (a, secrets, web, _) = accounts();
    assert_eq!(a.add("used0").unwrap_err(), "Epic: Sorry, that did not work.");
    assert!(a.view().contains("\"state\":\"failed\",\"scanned\":false,\"error\":\"Epic: Sorry, that did not work.\""));
    assert!(a.add("not a code").unwrap_err().starts_with("that is not a code"));
    assert_eq!(web.bodies.lock().unwrap().len(), 1, "nothing sent for what is not a code");
    assert!(secrets.0.lock().unwrap().is_empty());
    a.cancel_sign_in();
    assert!(a.view().contains("\"state\":\"idle\""), "the failure is put away");
    assert!(a.start_sign_in().is_err(), "no QR code for Epic");
    assert!(a.qr_svg().is_none());
}

#[test]
fn an_access_token_is_kept_while_it_lasts_then_renewed_with_the_new_refresh_token() {
    let (a, secrets, web, now) = accounts();
    a.add("good1").unwrap();
    assert_eq!(a.access_token(ONE).unwrap(), "a1", "the one the sign-in gave");
    assert_eq!(web.bodies.lock().unwrap().len(), 1);
    now.0.store(1_000 + 28_800_000 - 60_000, Ordering::SeqCst);
    assert_eq!(a.access_token(ONE).unwrap(), "a2", "renewed shortly before it runs out");
    assert!(web.bodies.lock().unwrap()[1].contains(&format!("refresh_token=r{ONE}-1&")));
    assert!(secrets.load(&token_name(ONE)).unwrap().unwrap().starts_with(&format!("r{ONE}-2\n")), "the new one kept");
    assert_eq!(a.access_token(ONE).unwrap(), "a2");
    assert!(a.access_token(TWO).unwrap_err().contains("is not signed in"));
}

#[test]
fn callers_at_once_near_expiry_spend_the_refresh_token_once() {
    let (a, _, web, now) = accounts();
    a.add("good1").unwrap();
    now.0.store(1_000 + 28_800_000 - 60_000, Ordering::SeqCst);
    let a = Arc::new(a);
    let callers: Vec<_> = (0..8)
        .map(|_| {
            let a = Arc::clone(&a);
            std::thread::spawn(move || a.access_token(ONE))
        })
        .collect();
    for c in callers {
        assert_eq!(c.join().unwrap().unwrap(), "a2");
    }
    assert_eq!(web.bodies.lock().unwrap().len(), 2, "the sign-in, then one refresh");
}

#[test]
fn a_refused_or_run_out_sign_in_needs_signing_in_again() {
    let (a, secrets, _, now) = accounts();
    a.add("good1").unwrap();
    secrets.save(&token_name(ONE), "revoked\n0\nPlayer One\n").unwrap();
    now.0.store(1_000 + 28_800_000, Ordering::SeqCst);
    assert_eq!(a.access_token(ONE).unwrap_err(), "Epic: Sorry, that did not work.");
    assert!(a.view().contains("\"refused\":true"));
    a.add("good1").unwrap();
    assert!(a.view().contains("\"refused\":false"), "a new sign-in renews it");
    now.0.store(1_000 + 1_987_200_000 + 28_800_000, Ordering::SeqCst);
    assert!(a.view().contains("\"refused\":true"), "its refresh token ran out");
    assert!(a.access_token(ONE).unwrap_err().contains("ran out"));
}

#[test]
fn an_account_is_signed_out_by_its_id_or_name() {
    let (a, secrets, _, _) = accounts();
    a.add("good1").unwrap();
    a.add("good2").unwrap();
    assert!(a.sign_out("nobody").is_err());
    a.sign_out("Player Two").unwrap();
    assert_eq!(a.accounts().len(), 1);
    a.sign_out(&ONE.to_uppercase()).unwrap();
    assert!(secrets.0.lock().unwrap().is_empty(), "nothing left behind");
    assert!(a.view().starts_with("{\"accounts\":[],\"account\":null"));
}

#[test]
fn the_built_in_client_is_named_until_another_is_set_and_after_it_goes() {
    let (a, secrets, web, _) = accounts();
    assert!(a.view().contains("\"client\":\"built-in\""));
    a.add("good1").unwrap();
    assert!(a.set_client(Some("not a client")).is_err());
    a.set_client(Some(" own1 : own2 ")).unwrap();
    assert_eq!(secrets.load(CLIENT).unwrap().as_deref(), Some("own1:own2"), "kept by lanstash itself");
    assert!(a.view().contains("\"client\":\"own\""));
    assert!(a.view().contains("clientId%3Down1"), "the sign-in page is the own client's");
    a.access_token(ONE).unwrap();
    a.set_client(None).unwrap();
    assert!(secrets.load(CLIENT).unwrap().is_none());
    a.add("good2").unwrap();
    // id:secret, own1:own2, id:secret.
    assert_eq!(*web.clients.lock().unwrap(), ["basic aWQ6c2VjcmV0", "basic b3duMTpvd24y", "basic aWQ6c2VjcmV0"]);
    assert_eq!(a.accounts().len(), 2, "signed-in accounts stay");
}
