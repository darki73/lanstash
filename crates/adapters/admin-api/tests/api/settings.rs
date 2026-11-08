//! Tests for `lanstash_adapter_admin_api::api::settings`: the settings and
//! restart endpoints.

use std::sync::atomic::{AtomicBool, Ordering};

use lanstash_ports::settings::Tunables;

use super::*;

#[derive(Default)]
struct Kept(Mutex<Vec<(String, String)>>);

impl Tunables for Kept {
    fn view(&self) -> String {
        format!("{:?}\n", self.0.lock().unwrap())
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        if key == "listen" {
            return Err("`listen` is set in the configuration file only".into());
        }
        self.0.lock().unwrap().push((key.into(), value.into()));
        Ok(())
    }
    fn reset(&self, key: &str) -> Result<(), String> {
        self.0.lock().unwrap().retain(|(k, _)| k != key);
        Ok(())
    }
}

#[test]
fn settings_are_shown_changed_and_reset() {
    let kept = Arc::new(Kept::default());
    let restarted = Arc::new(AtomicBool::new(false));
    let r = Arc::clone(&restarted);
    let a =
        api().with_settings(Arc::clone(&kept) as Arc<dyn Tunables>, Arc::new(move || r.store(true, Ordering::SeqCst)));
    assert_eq!(get(&a, "GET", "/api/v1/settings").body, "[]\n");
    let set = get(&a, "PUT", "/api/v1/settings/blizzard_tags?value=Windows%2Cx86_64%2CenUS");
    assert_eq!((set.status, set.body.as_str()), (200, "[(\"blizzard_tags\", \"Windows,x86_64,enUS\")]\n"));
    let refused = get(&a, "PUT", "/api/v1/settings/listen?value=0.0.0.0%3A81");
    assert_eq!(refused.status, 400);
    assert!(refused.body.contains("configuration file only"), "{}", refused.body);
    assert_eq!(get(&a, "PUT", "/api/v1/settings/max_size").status, 400, "no value");
    assert_eq!(get(&a, "DELETE", "/api/v1/settings/blizzard_tags").body, "[]\n");
    let go = get(&a, "POST", "/api/v1/restart");
    assert_eq!((go.status, go.body.as_str()), (202, "{\"restarting\":true}\n"));
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while !restarted.load(Ordering::SeqCst) && std::time::Instant::now() < until {
        std::thread::yield_now();
    }
    assert!(restarted.load(Ordering::SeqCst));
    assert_eq!(get(&api(), "GET", "/api/v1/settings").status, 404, "none without settings");
}

#[test]
fn a_restart_waits_for_a_running_prefill_unless_forced() {
    let jobs = Jobs::new(Ok(()), 1);
    let a = api()
        .with_platforms(vec![Arc::new(Echo { id: "steam", views: false, jobs: Some(Arc::clone(&jobs)) })])
        .with_settings(Arc::new(Kept::default()), Arc::new(|| {}));
    get(&a, "POST", "/api/v1/prefill/steam/7");
    let busy = get(&a, "POST", "/api/v1/restart");
    assert_eq!(busy.status, 409);
    assert!(busy.body.contains("a prefill is running"), "{}", busy.body);
    assert_eq!(get(&a, "POST", "/api/v1/restart?force=1").status, 202);
}

#[test]
fn query_values_are_decoded() {
    use lanstash_adapter_admin_api::api::settings::decode;
    assert_eq!(decode("a%2Cb+c").as_deref(), Some("a,b c"));
    assert_eq!(decode("00%3A00-24%3A00").as_deref(), Some("00:00-24:00"));
    assert_eq!(decode("%zz"), None);
    assert_eq!(decode("%E2%82%AC").as_deref(), Some("€"));
}
