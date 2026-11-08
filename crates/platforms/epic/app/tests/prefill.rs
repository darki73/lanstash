//! Tests for `lanstash_epic_app::prefill`, against a scripted Epic.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash_app::prefill::Planner;
use lanstash_domain::checksum::sha1::sha1;
use lanstash_epic_app::accounts::{EpicAccounts, Wiring};
use lanstash_epic_app::library::EpicLibrary;
use lanstash_epic_app::prefill::EpicPlanner;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::prefill::StartError;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};

const ONE: &str = "11111111111111111111111111111111";
const DATA: &[u8] = b"some chunk data";

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

/// A manifest of one chunk of [`DATA`], laid out as `lanstash_epic::manifest` reads it.
fn manifest() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&9u32.to_le_bytes());
    data.push(0);
    data.extend_from_slice(&18u32.to_le_bytes());
    let mut list = vec![0u8];
    list.extend_from_slice(&1u32.to_le_bytes());
    [1u32, 2, 3, 4].iter().for_each(|g| list.extend_from_slice(&g.to_le_bytes()));
    list.extend_from_slice(&0xABu64.to_le_bytes());
    list.extend_from_slice(&sha1(DATA));
    list.push(5);
    list.extend_from_slice(&1u32.to_le_bytes());
    list.extend_from_slice(&56u64.to_le_bytes());
    data.extend_from_slice(&(4 + list.len() as u32).to_le_bytes());
    data.extend_from_slice(&list);
    let mut b = Vec::new();
    [0x44BE_C00Cu32, 41, data.len() as u32, data.len() as u32]
        .iter()
        .for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
    b.extend_from_slice(&sha1(&data));
    b.push(0);
    b.extend_from_slice(&21u32.to_le_bytes());
    b.extend_from_slice(&data);
    b
}

struct Epic;

impl Web for Epic {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        let ok = |b: Vec<u8>| Ok(WebResponse { status: 200, body: b });
        let t = req.target;
        if req.method == "POST" {
            return ok(format!(r#"{{"access_token":"a","expires_in":28800,"refresh_token":"r","account_id":"{ONE}","displayName":"P"}}"#).into_bytes());
        }
        if t.starts_with("/launcher/api/public/assets/Windows") {
            return ok(br#"[{"appName":"app","buildVersion":"7","catalogItemId":"c1","namespace":"ns"}]"#.to_vec());
        }
        if t.starts_with("/library/") {
            return ok(br#"{"records":[]}"#.to_vec());
        }
        if t.starts_with("/launcher/api/public/assets/v2/") {
            return ok(br#"{"elements":[{"manifests":[
                {"uri":"https://cdn-a.example/Builds/Org/o-1/dir1/default/M.manifest","queryParams":[]},
                {"uri":"https://cdn-b.example/Builds/Org/o-1/dir1/default/M.manifest","queryParams":[{"name":"t","value":"1"}]}]}]}"#.to_vec());
        }
        if t.starts_with("/catalog/") {
            return ok(br#"{"c1":{"title":"A Game","categories":[{"path":"games"}]}}"#.to_vec());
        }
        if req.host == "cdn-a.example" {
            return Ok(WebResponse { status: 403, body: Vec::new() });
        }
        assert_eq!((req.host, t, req.port), ("cdn-b.example", "/Builds/Org/o-1/dir1/default/M.manifest?t=1", 443));
        ok(manifest())
    }
}

fn planner() -> EpicPlanner {
    let web = Arc::new(Epic);
    let wiring = Wiring {
        web: web.clone(),
        secrets: Arc::new(Memory::default()),
        clock: Arc::new(FixedClock(1)),
        client_id: "id".into(),
        client_secret: "s".into(),
    };
    let accounts = Arc::new(EpicAccounts::new(wiring));
    accounts.add("good").unwrap();
    let library = Arc::new(EpicLibrary::new(accounts, web.clone(), Arc::new(FixedClock(1))));
    library.refresh().unwrap();
    EpicPlanner::new(library, web)
}

fn chunk_file(data: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    [0xB1FE_3AA2u32, 3, 41, data.len() as u32].iter().for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
    b.extend_from_slice(&[0; 24]);
    b.push(0);
    b.extend_from_slice(data);
    b
}

#[test]
fn a_game_is_prefilled_from_its_manifests_chunks_each_checked() {
    let p = planner();
    assert_eq!(p.admit("app"), Ok(()));
    assert_eq!(p.admit("dir1"), Ok(()), "by its download directory too, once known");
    assert_eq!(p.admit("nothing"), Err(StartError::NotAGame));
    let seen = Mutex::new(String::new());
    let work = p.work("app", false, &|v| *seen.lock().unwrap() = v.to_string()).unwrap();
    assert_eq!(*seen.lock().unwrap(), "7");
    assert_eq!(work.hosts, ["cdn-a.example", "cdn-b.example"]);
    assert_eq!(work.jobs.len(), 1);
    assert_eq!(
        work.jobs[0].target,
        "/Builds/Org/o-1/dir1/default/ChunksV4/05/00000000000000AB_00000001000000020000000300000004.chunk"
    );
    assert_eq!(work.bytes, 56);
    assert!(work.check_cdn);
    let mut good = (work.check)(0, 0, 0, &[]);
    good.feed(&chunk_file(DATA));
    assert!(good.finish());
    let mut bad = (work.check)(0, 0, 0, &[]);
    bad.feed(&chunk_file(b"other"));
    assert!(!bad.finish());
    assert!(p.work("app", true, &|_| {}).is_err(), "no pre-release");
}
