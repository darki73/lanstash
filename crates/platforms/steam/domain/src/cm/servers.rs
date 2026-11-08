//! Which CM servers to try: `ISteamDirectory/GetCMListForConnect` answers
//! `{"response": {"serverlist": [{"endpoint": "host:port", "type":
//! "websockets", "load": .., ..}, ..]}}`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use lanstash_domain::json::{self, Value};

/// Where the list is asked for: host and path (cell 0: let Steam pick by address).
pub const DIRECTORY: (&str, &str) = ("api.steampowered.com", "/ISteamDirectory/GetCMListForConnect/v1/?cellid=0");

/// The WebSocket path on a CM.
pub const PATH: &str = "/cmsocket/";

/// The WebSocket servers in the answer, in Steam's order (best first): `(host, port)`.
///
/// # Errors
/// Not the directory's JSON.
pub fn websockets(answer: &[u8]) -> Result<Vec<(String, u16)>, String> {
    let text = core::str::from_utf8(answer).map_err(|_| "the CM list is not UTF-8".to_string())?;
    let doc = json::parse(text).map_err(|e| alloc::format!("the CM list: {e:?}"))?;
    let list = doc.get("response").and_then(|r| r.get("serverlist")).ok_or("the CM list has no serverlist")?;
    Ok(list
        .as_array()
        .iter()
        .filter(|s| s.get("type").and_then(Value::as_str) == Some("websockets"))
        .filter_map(|s| {
            let (host, port) = s.get("endpoint")?.as_str()?.rsplit_once(':')?;
            Some((host.to_string(), port.parse().ok()?))
        })
        .collect())
}
