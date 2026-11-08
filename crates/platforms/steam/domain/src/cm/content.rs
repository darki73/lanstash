//! Steam's content servers, as service calls once logged on: which servers
//! to fetch depot content from (`GetServersForSteamPipe`), and the code a
//! manifest download needs (`GetManifestRequestCode`, which changes every
//! few minutes).
//!
//! Field numbers follow Steam's
//! `steammessages_contentsystem.steamclient.proto`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::message::CmError;
use super::proto::{Writer, fields};

/// Service method names.
pub mod method {
    pub const GET_SERVERS_FOR_STEAM_PIPE: &str = "ContentServerDirectory.GetServersForSteamPipe#1";
    pub const GET_MANIFEST_REQUEST_CODE: &str = "ContentServerDirectory.GetManifestRequestCode#1";
}

/// `CContentServerDirectory_GetServersForSteamPipe_Request` (1 `cell_id`, 2 `max_servers`).
#[must_use]
pub fn servers_request(cell_id: u32, max: u32) -> Vec<u8> {
    Writer::new().varint(1, u64::from(cell_id)).varint(2, u64::from(max)).finish()
}

/// A content server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentServer {
    /// 1: `SteamCache`, `CDN`, ...
    pub kind: String,
    /// 8.
    pub host: String,
    /// 9: the name to ask for (the Host header).
    pub vhost: String,
    /// 12: `mandatory`, `optional` or empty.
    pub https: String,
    /// 13: when not empty, the only apps it serves.
    pub allowed_apps: Vec<u32>,
}

/// `CContentServerDirectory_GetServersForSteamPipe_Response` (1 `servers`), in Steam's order.
///
/// # Errors
/// Not a protobuf.
pub fn servers(body: &[u8]) -> Result<Vec<ContentServer>, CmError> {
    let mut out = Vec::new();
    for f in fields(body) {
        let (n, v) = f.map_err(|()| CmError::Malformed("content servers"))?;
        if n != 1 {
            continue;
        }
        let mut s = ContentServer {
            kind: String::new(),
            host: String::new(),
            vhost: String::new(),
            https: String::new(),
            allowed_apps: Vec::new(),
        };
        for g in fields(v.bytes().unwrap_or_default()).filter_map(Result::ok) {
            let text = || g.1.str().unwrap_or_default().to_string();
            match g.0 {
                1 => s.kind = text(),
                8 => s.host = text(),
                9 => s.vhost = text(),
                12 => s.https = text(),
                13 => s.allowed_apps.extend(g.1.int().map(|x| x as u32)),
                _ => {}
            }
        }
        out.push(s);
    }
    Ok(out)
}

/// `CContentServerDirectory_GetManifestRequestCode_Request` (1 `app_id`, 2
/// `depot_id`, 3 `manifest_id`, 4 `app_branch`).
#[must_use]
pub fn request_code_request(app: u32, depot: u32, manifest: u64, branch: &str) -> Vec<u8> {
    Writer::new().varint(1, u64::from(app)).varint(2, u64::from(depot)).varint(3, manifest).string(4, branch).finish()
}

/// `..._GetManifestRequestCode_Response` (1 `manifest_request_code`); `None` when Steam gave none.
///
/// # Errors
/// Not a protobuf.
pub fn request_code(body: &[u8]) -> Result<Option<u64>, CmError> {
    let mut code = None;
    for f in fields(body) {
        if let (1, v) = f.map_err(|()| CmError::Malformed("manifest request code"))? {
            code = v.int().filter(|c| *c != 0);
        }
    }
    Ok(code)
}
