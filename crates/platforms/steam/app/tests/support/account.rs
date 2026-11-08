//! A scripted Steam account for the library, manifest and prefill tests:
//! logon, one license (package 37), the package's KeyValues (app 100), app
//! 100's info (depots 101 and 102 for Windows, 103 for Linux, a shared one),
//! a content server, request codes; and the depots' manifests on a fake CDN.
//! On request the account also owns DLC 200, whose content is its own depot 201.

#![allow(dead_code)]

use lanstash_app::test_support::{FakeResponse, FakeUpstream};
use lanstash_domain::checksum::crc::crc32;
use lanstash_steam_domain::cm::content::method;
use lanstash_steam_domain::cm::message::{Header, Message, emsg};
use lanstash_steam_domain::cm::pics::emsg as pics;
use lanstash_steam_domain::cm::proto::{Writer, fields};

use super::{Script, answer, multi};

pub const STEAMID: u64 = 76_561_197_960_287_930;
pub const CDN: &str = "cdn.example";

pub const APP_INFO: &str = r#""appinfo" {
    "appid" "100"
    "common" { "name" "Some Game" "type" "Game" }
    "depots" {
        "228989" { "config" { "oslist" "windows" } "depotfromapp" "228980" }
        "101" { "manifests" { "public" { "gid" "11" "size" "900" "download" "300" } } }
        "102" { "config" { "oslist" "windows" } "manifests" { "public" { "gid" "12" "size" "90" "download" "40" } } }
        "103" { "config" { "oslist" "linux" } "manifests" { "public" { "gid" "13" "size" "1" "download" "1" } } }
        "branches" { "public" { "buildid" "555" } }
    }
}"#;

/// A DLC of app 100 whose content is a depot of its own (201), as Cyberpunk
/// 2077: Phantom Liberty's is.
pub const DLC_INFO: &str = r#""appinfo" {
    "appid" "200"
    "common" { "name" "Some Expansion" "type" "DLC" "parent" "100" }
    "depots" {
        "201" { "manifests" { "public" { "gid" "21" "size" "50" "download" "20" } } }
    }
}"#;

fn entry(b: &mut Vec<u8>, kind: u8, key: &str, value: &[u8]) {
    b.push(kind);
    b.extend_from_slice(key.as_bytes());
    b.push(0);
    b.extend_from_slice(value);
}

/// Binary KeyValues of package 37 (after its 4-byte prefix): app 100, its depots
/// 101, 102 and 103 (not the shared 228989: another package grants that); with the
/// DLC, app 200 and depot 201 too.
fn package(dlc: bool) -> Vec<u8> {
    let (apps, depots): (&[i32], &[i32]) =
        if dlc { (&[100, 200], &[101, 102, 103, 201]) } else { (&[100], &[101, 102, 103]) };
    package_of(37, apps, depots)
}

/// Binary KeyValues of package `id` (after its 4-byte prefix) granting `apps` and `depots`.
pub fn package_of(id: u32, apps: &[i32], depots: &[i32]) -> Vec<u8> {
    let mut b = vec![1, 0, 0, 0];
    entry(&mut b, 0x00, &id.to_string(), &[]);
    entry(&mut b, 0x00, "appids", &[]);
    for (i, app) in apps.iter().enumerate() {
        entry(&mut b, 0x02, &i.to_string(), &app.to_le_bytes());
    }
    b.push(0x08);
    entry(&mut b, 0x00, "depotids", &[]);
    for (i, depot) in depots.iter().enumerate() {
        entry(&mut b, 0x02, &i.to_string(), &depot.to_le_bytes());
    }
    b.extend_from_slice(&[0x08, 0x08, 0x08]);
    b
}

pub fn reply(to: &Message, emsg: u32, body: Vec<u8>) -> Vec<u8> {
    Message::new(emsg, Header { job_target: to.header.job_source, ..Header::default() }, body).encode()
}

/// Steam for an account owning app 100.
pub fn account() -> Script {
    account_with(false)
}

/// Steam for an account owning app 100, and its DLC 200 when asked.
pub fn account_with(dlc: bool) -> Script {
    Box::new(move |m| match m.emsg {
        emsg::CLIENT_LOGON => {
            let h = Header { steamid: STEAMID, session: 5, ..Header::default() };
            let ok = Writer::new().varint(1, 1).varint(3, 9).varint(7, 127).finish();
            let license = Writer::new().varint(1, 37).varint(17, 5).finish();
            vec![multi(&[
                Message::new(emsg::CLIENT_LOG_ON_RESPONSE, h, ok).encode(),
                Message::new(pics::CLIENT_LICENSE_LIST, Header::default(), Writer::new().bytes(2, &license).finish())
                    .encode(),
            ])]
        }
        pics::PICS_ACCESS_TOKEN_REQUEST => {
            vec![reply(
                m,
                pics::PICS_ACCESS_TOKEN_RESPONSE,
                Writer::new().bytes(3, &Writer::new().varint(1, 100).varint(2, 7).finish()).finish(),
            )]
        }
        pics::PICS_PRODUCT_INFO_REQUEST => {
            let packages = fields(&m.body).filter_map(Result::ok).any(|(n, _)| n == 1);
            if packages {
                let p = Writer::new().varint(1, 37).bytes(5, &package(dlc)).finish();
                vec![reply(m, pics::PICS_PRODUCT_INFO_RESPONSE, Writer::new().bytes(3, &p).finish())]
            } else {
                // Two parts: the first only says more is coming.
                let a = Writer::new().varint(1, 100).bytes(5, APP_INFO.as_bytes()).finish();
                let mut last = Writer::new().bytes(1, &a);
                if dlc {
                    last = last.bytes(1, &Writer::new().varint(1, 200).bytes(5, DLC_INFO.as_bytes()).finish());
                }
                vec![
                    reply(m, pics::PICS_PRODUCT_INFO_RESPONSE, Writer::new().bool(6, true).finish()),
                    reply(m, pics::PICS_PRODUCT_INFO_RESPONSE, last.finish()),
                ]
            }
        }
        emsg::SERVICE_METHOD_CALL_FROM_CLIENT => match m.header.target_job_name.as_deref() {
            Some(method::GET_SERVERS_FOR_STEAM_PIPE) => {
                let s = Writer::new().string(1, "CDN").string(8, CDN).string(9, CDN).finish();
                vec![answer(m, 1, Writer::new().bytes(1, &s).finish())]
            }
            Some(method::GET_MANIFEST_REQUEST_CODE) => vec![answer(m, 1, Writer::new().varint(1, 1234).finish())],
            _ => vec![answer(m, 2, vec![])],
        },
        _ => Vec::new(),
    })
}

/// A manifest of `depot` (`gid`) whose chunks are `chunks` (sha byte, compressed size), zipped as stored.
pub fn manifest(depot: u32, gid: u64, chunks: &[(u8, u32)]) -> Vec<u8> {
    let mut file = Writer::new().string(1, "f").varint(2, 1);
    for (sha, size) in chunks {
        let c = Writer::new().bytes(1, &[*sha; 20]).varint(4, u64::from(*size) * 2).varint(5, u64::from(*size));
        file = file.bytes(6, &c.finish());
    }
    let payload = Writer::new().bytes(1, &file.finish()).finish();
    let metadata = Writer::new().varint(1, u64::from(depot)).varint(2, gid).finish();
    let mut data = Vec::new();
    for (magic, body) in [(0x71F6_17D0u32, payload), (0x1F48_12BE, metadata)] {
        data.extend_from_slice(&magic.to_le_bytes());
        data.extend_from_slice(&(body.len() as u32).to_le_bytes());
        data.extend(body);
    }
    data.extend_from_slice(&0x32C4_15ABu32.to_le_bytes());
    stored_zip(&data)
}

fn stored_zip(data: &[u8]) -> Vec<u8> {
    let (crc, len) = (crc32(data), data.len() as u32);
    let mut z = Vec::new();
    z.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    z.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&[1, 0, 0, 0, b'z']);
    z.extend_from_slice(data);
    let central = z.len() as u32;
    z.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    z.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.push(b'z');
    let size = z.len() as u32 - central;
    z.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    z.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    z.extend_from_slice(&size.to_le_bytes());
    z.extend_from_slice(&central.to_le_bytes());
    z.extend_from_slice(&[0, 0]);
    z
}

/// The CDN with depots 101 (two chunks, one shared) and 102's manifests.
pub fn cdn() -> FakeUpstream {
    let up = FakeUpstream::default();
    up.route(
        CDN,
        "/depot/101/manifest/11/5/1234",
        FakeResponse::ok(&manifest(101, 11, &[(1, 100), (2, 200), (1, 100)]), &[]),
    );
    up.route(CDN, "/depot/102/manifest/12/5/1234", FakeResponse::ok(&manifest(102, 12, &[(3, 40)]), &[]));
    up.route(CDN, "/depot/103/manifest/13/5/1234", FakeResponse::ok(&manifest(103, 13, &[(4, 1)]), &[]));
    up.route(CDN, "/depot/201/manifest/21/5/1234", FakeResponse::ok(&manifest(201, 21, &[(5, 20)]), &[]));
    up
}

/// Opens a fresh scripted account at every dial (owning the DLC too when
/// asked); lists one CM.
pub struct AccountDialer(pub bool);

impl lanstash_ports::link::Dialer for AccountDialer {
    fn dial(&self, _: &str, _: u16, _: &str) -> Result<Box<dyn lanstash_ports::link::Link>, String> {
        Ok(super::steam(account_with(self.0)).0)
    }

    fn get(&self, _: &str, _: &str) -> Result<Vec<u8>, String> {
        Ok(br#"{"response":{"serverlist":[{"endpoint":"cm.example:443","type":"websockets"}]}}"#.to_vec())
    }
}

/// The services of a signed-in account (or not) with the fake CDN.
pub fn services(signed_in: bool) -> (lanstash_steam_app::session::Services, std::sync::Arc<FakeUpstream>) {
    services_with(signed_in, false)
}

/// The services of a signed-in account (or not) with the fake CDN, owning
/// the DLC too when asked.
pub fn services_with(
    signed_in: bool,
    dlc: bool,
) -> (lanstash_steam_app::session::Services, std::sync::Arc<FakeUpstream>) {
    use lanstash_ports::secrets::Secrets;
    let secrets = super::Memory::default();
    if signed_in {
        secrets.save(lanstash_steam_app::login::REFRESH_TOKEN, "REFRESH").unwrap();
        secrets.save(lanstash_steam_app::login::ACCOUNT, "someone").unwrap();
    }
    let up = std::sync::Arc::new(cdn());
    let s = lanstash_steam_app::session::Services::new(
        std::sync::Arc::new(AccountDialer(dlc)),
        std::sync::Arc::new(secrets),
        up.clone(),
        std::sync::Arc::new(lanstash_ports::clock::FixedClock(0)),
    );
    (s, up)
}
