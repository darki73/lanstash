//! Tests for `lanstash_adapter_admin_api::api::dns`: the DNS page's endpoints,
//! against a setup and a server in memory.

use lanstash_app::dns::DnsLink;
use lanstash_ports::dns::{DnsConfig, DnsKind, DnsServer, DnsSetup, Lookup, Rewrite};

use super::*;

#[derive(Default)]
struct Server(Mutex<Vec<Rewrite>>);

impl DnsServer for Server {
    fn kind(&self) -> &str {
        "AdGuard Home"
    }
    fn rewrites(&self) -> Result<Vec<Rewrite>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn add(&self, r: &Rewrite) -> Result<(), String> {
        self.0.lock().unwrap().push(r.clone());
        Ok(())
    }
    fn remove(&self, r: &Rewrite) -> Result<(), String> {
        self.0.lock().unwrap().retain(|x| x != r);
        Ok(())
    }
    fn lookups(&self, _: usize) -> Result<Vec<Lookup>, String> {
        Ok(vec![])
    }
}

#[derive(Default)]
struct Setup {
    kept: Mutex<Option<(DnsConfig, String)>>,
    server: Arc<Server>,
}

impl DnsSetup for Setup {
    fn kinds(&self) -> Vec<DnsKind> {
        vec![DnsKind { id: "adguard".into(), name: "AdGuard Home".into(), port: 3000, https_port: 443, user: true }]
    }
    fn config(&self) -> Option<DnsConfig> {
        self.kept.lock().unwrap().as_ref().map(|(c, _)| c.clone())
    }
    fn password(&self) -> Option<String> {
        self.kept.lock().unwrap().as_ref().map(|(_, p)| p.clone())
    }
    fn save(&self, config: &DnsConfig, password: &str) -> Result<(), String> {
        *self.kept.lock().unwrap() = Some((config.clone(), password.into()));
        Ok(())
    }
    fn forget(&self) -> Result<(), String> {
        *self.kept.lock().unwrap() = None;
        Ok(())
    }
    fn connect(&self, config: &DnsConfig, _: &str) -> Option<Arc<dyn DnsServer>> {
        (config.kind == "adguard").then(|| Arc::clone(&self.server) as Arc<dyn DnsServer>)
    }
}

fn dns_api() -> (AdminApi, Arc<Setup>) {
    let setup = Arc::new(Setup::default());
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(0)))
            .unwrap();
    let cache = Arc::new(Cache::new(
        Arc::new(store),
        HostRules::default(),
        &Profiles::default(),
        Arc::new(NoEvents),
        ServeOptions::default(),
    ));
    let wanted = vec!["*.steamcontent.com".to_owned(), "lancache.steamcontent.com".to_owned()];
    let link = DnsLink::new(Arc::clone(&setup) as Arc<dyn DnsSetup>, cache, HostRules::default(), wanted);
    (api().with_dns(Arc::new(link)), setup)
}

/// A request with the password in its header.
fn with_password(api: &AdminApi, method: &str, target: &str, password: &str) -> Answer {
    let req = RequestHead {
        method: method.into(),
        target: target.into(),
        minor_version: 1,
        headers: vec![("x-lanstash-password".into(), password.into())],
    };
    let mut c = Capture::default();
    api.handle(&req, &mut ConnState::default(), &mut c).unwrap();
    Answer { status: c.status, headers: c.headers, body: String::from_utf8_lossy(&c.body).into_owned() }
}

const SETUP: &str = "/api/v1/dns/setup?kind=adguard&host=10.0.0.53&port=3000&user=lanstash&answer=10.0.0.210";

#[test]
fn the_dns_server_is_set_up_tested_kept_and_forgotten() {
    let (a, setup) = dns_api();
    let first = get(&a, "GET", "/api/v1/dns");
    assert!(
        first
            .body
            .contains(r#""kinds":[{"id":"adguard","name":"AdGuard Home","port":3000,"https_port":443,"user":true}]"#),
        "{}",
        first.body
    );
    assert!(first.body.contains(r#""config":null,"password_set":false"#));
    assert!(first.body.contains(r#""report":null,"report_error":"no DNS server is set up yet""#));
    assert_eq!(get(&a, "PUT", SETUP).status, 400, "no password, none kept");
    let test = with_password(&a, "PUT", &format!("{SETUP}&test=1"), "s3cret");
    assert_eq!(test.status, 200, "{}", test.body);
    assert!(test.body.contains(r#""done":{"tested":true,"kept":false,"server":"AdGuard Home","rewrites":0}"#));
    assert!(setup.config().is_none(), "a test keeps nothing");
    let kept = with_password(&a, "PUT", SETUP, "s3cret");
    assert!(kept.body.contains(r#""config":{"kind":"adguard","host":"10.0.0.53","port":3000,"https":false,"pin":null,"unchecked":false,"user":"lanstash","answer":"10.0.0.210","via":null},"password_set":true"#), "{}", kept.body);
    assert!(kept.body.contains(r#""missing":["*.steamcontent.com","lancache.steamcontent.com"]"#));
    assert!(!kept.body.contains("s3cret"), "the password is never shown");
    let applied = get(&a, "POST", "/api/v1/dns/apply");
    assert!(applied.body.contains(r#""done":{"added":2,"deduplicated":0}"#), "{}", applied.body);
    assert!(applied.body.contains(r#""missing":[]"#));
    let bad = with_password(&a, "PUT", "/api/v1/dns/setup?kind=pihole&host=x&port=1&user=u", "p");
    assert!(bad.status == 400 && bad.body.contains("unknown DNS server"), "{}", bad.body);
    let gone = get(&a, "DELETE", "/api/v1/dns/setup");
    assert!(gone.body.contains(r#""config":null"#) && setup.password().is_none());
    assert_eq!(get(&api(), "GET", "/api/v1/dns").status, 404, "none without a DNS link");
}
