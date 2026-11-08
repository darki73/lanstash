//! Steam's connection manager, live (ignored: needs the internet): its server
//! list over HTTPS, a WebSocket to one, ClientHello and an anonymous logon,
//! and what Steam answers.

use std::time::Duration;

use lanstash_adapter_steam::wss::{Wss, https_get};
use lanstash_domain::json::{self, Value};

fn varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// A Steam message: EMsg with the protobuf flag, its header (steamid, session 0), its body.
fn message(emsg: u32, steamid: u64, body: &[u8]) -> Vec<u8> {
    let mut header = vec![0x09];
    header.extend_from_slice(&steamid.to_le_bytes());
    let mut m = (emsg | 0x8000_0000).to_le_bytes().to_vec();
    m.extend_from_slice(&(header.len() as u32).to_le_bytes());
    m.extend_from_slice(&header);
    m.extend_from_slice(body);
    m
}

#[test]
#[ignore = "talks to Steam"]
fn steam_answers_a_logon_over_wss() {
    let t = Duration::from_secs(15);
    let list = https_get("api.steampowered.com", "/ISteamDirectory/GetCMListForConnect/v1/?cellid=0", t).unwrap();
    let doc = json::parse(&String::from_utf8_lossy(&list)).unwrap();
    let servers = doc.get("response").and_then(|r| r.get("serverlist")).map(Value::as_array).unwrap_or_default();
    let ws = servers
        .iter()
        .find(|s| s.get("type").and_then(Value::as_str) == Some("websockets"))
        .and_then(|s| s.get("endpoint")?.as_str())
        .expect("a websockets endpoint");
    println!("{} servers; using {ws}", servers.len());
    let (host, port) = ws.rsplit_once(':').unwrap();
    let mut c = Wss::connect(host, port.parse().unwrap(), "/cmsocket/", t).unwrap();
    let anon = 0x01A0_0000_0000_0000u64; // universe public, account type anonymous user
    let mut hello = vec![0x08];
    varint(65580, &mut hello); // protocol_version
    c.send(&message(9805, 0, &hello)).unwrap(); // ClientHello
    let mut logon = vec![0x08];
    varint(65580, &mut logon);
    c.send(&message(5514, anon, &logon)).unwrap(); // ClientLogon, anonymous
    for _ in 0..3 {
        let m = c.recv().unwrap().expect("a message within the timeout");
        let emsg = u32::from_le_bytes(m[..4].try_into().unwrap()) & 0x7FFF_FFFF;
        println!("received EMsg {emsg} ({} bytes)", m.len());
        if emsg == 751 || emsg == 1 {
            return; // ClientLogOnResponse, or a Multi carrying it
        }
    }
    panic!("no logon response");
}
