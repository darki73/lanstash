//! Depot manifests: the content servers to ask (Steam's list, those that
//! answer plain HTTP first), a request code for the manifest, then the
//! manifest itself through the [`Upstream`] port, parsed. Downloading a
//! manifest is what the Steam client does too (lanstash passes manifests
//! through uncached). [`chunks`] serves a manifest's chunk list from those
//! kept ([`ChunkLists`]) when it can, so a restart downloads only new builds'.

use lanstash_ports::upstream::{Upstream, UpstreamRequest};
use lanstash_steam_domain::cm::content::{self, ContentServer};
use lanstash_steam_domain::manifest::{Chunk, MAX_MANIFEST, Manifest, parse};

use crate::chunk_lists::ChunkLists;
use crate::connection::Connection;
use crate::library::{InstallDepot, Library};
use crate::session::Services;

/// How many content servers to ask Steam for.
const SERVERS: u32 = 20;

/// The user agent Steam's own client sends to content servers.
pub const USER_AGENT: &str = "Valve/Steam HTTP Client 1.0";

/// Content servers for cell `cell`, those that serve plain HTTP first (CDNs for anyone, not caches for some apps).
///
/// # Errors
/// The connection, or Steam's answer.
pub fn servers(conn: &mut Connection, cell: u32) -> Result<Vec<ContentServer>, String> {
    let answer = conn
        .call(content::method::GET_SERVERS_FOR_STEAM_PIPE, content::servers_request(cell, SERVERS))
        .map_err(|e| e.to_string())?;
    let mut list: Vec<ContentServer> = content::servers(&answer.body)
        .map_err(|e| format!("content servers: {e:?}"))?
        .into_iter()
        .filter(|s| s.allowed_apps.is_empty() && !s.host.is_empty())
        .collect();
    list.sort_by_key(|s| s.https == "mandatory");
    if list.is_empty() {
        return Err("Steam listed no content servers".into());
    }
    Ok(list)
}

/// The manifest `gid` of `depot` (of `app`), from the first of `servers` that has it.
///
/// # Errors
/// No request code, no server with it, or a manifest that does not parse.
pub fn fetch(
    conn: &mut Connection,
    upstream: &dyn Upstream,
    servers: &[ContentServer],
    (app, depot, gid): (u32, u32, u64),
) -> Result<Manifest, String> {
    let answer = conn
        .call(content::method::GET_MANIFEST_REQUEST_CODE, content::request_code_request(app, depot, gid, "public"))
        .map_err(|e| format!("depot {depot}: {e}"))?;
    let code = content::request_code(&answer.body)
        .map_err(|e| format!("depot {depot}: {e:?}"))?
        .ok_or(format!("depot {depot}: Steam gave no manifest request code"))?;
    let target = format!("/depot/{depot}/manifest/{gid}/5/{code}");
    let mut last = format!("depot {depot}: no content server");
    for s in servers.iter().take(3) {
        match get(upstream, &s.host, &target).and_then(|body| {
            let m = parse(&body).map_err(|e| format!("manifest: {e:?}"))?;
            // Kept as this manifest's chunk list: it must be the one asked for.
            if (m.depot, m.gid) == (depot, gid) {
                Ok(m)
            } else {
                Err(format!("got manifest {} of depot {}, not {gid}", m.gid, m.depot))
            }
        }) {
            Ok(m) => return Ok(m),
            Err(e) => last = format!("depot {depot} from {}: {e}", s.host),
        }
    }
    Err(last)
}

/// The chunks of an install's `depots`, as (depot, chunk): each depot's manifest read as an
/// account whose licenses grant it, on that account's session ([`Library::by_account`]);
/// a depot no signed-in account is granted is left out. Lists are kept when `keep`.
///
/// # Errors
/// A session, or a manifest that could not be read.
pub fn install_chunks(
    services: &Services,
    servers: &mut Option<Servers>,
    lib: &Library,
    depots: &[InstallDepot<'_>],
    keep: bool,
) -> Result<Vec<(u32, Chunk)>, String> {
    let mut out = Vec::new();
    for (account, group) in lib.by_account(depots, &services.accounts()) {
        let part = services.with_session_as(&account, |conn: &mut Connection, on| {
            let s = servers.get_or_insert_with(|| Servers::new(on.cell_id));
            let mut part = Vec::new();
            for (owner, d) in &group {
                let gid = d.manifest.ok_or(format!("depot {} has no manifest", d.id))?;
                let (up, lists) = (services.upstream.as_ref(), &services.lists);
                part.extend(chunks(conn, up, lists, s, (*owner, d.id, gid), keep)?.into_iter().map(|c| (d.id, c)));
            }
            Ok(part)
        })?;
        out.extend(part);
    }
    Ok(out)
}

/// Content servers, asked for at most once, and only when a manifest must be downloaded.
#[derive(Debug)]
pub struct Servers {
    cell: u32,
    list: Option<Vec<ContentServer>>,
}

impl Servers {
    #[must_use]
    pub fn new(cell: u32) -> Self {
        Self { cell, list: None }
    }

    fn get(&mut self, conn: &mut Connection) -> Result<&[ContentServer], String> {
        if self.list.is_none() {
            self.list = Some(servers(conn, self.cell)?);
        }
        Ok(self.list.as_deref().unwrap_or_default())
    }
}

/// The unique chunks of `depot`'s manifest `gid` (of `app`): the list kept in `lists` when it is
/// that manifest's, else the manifest downloaded (and its list kept when `keep`).
///
/// # Errors
/// As [`fetch`], when it has to download.
pub fn chunks(
    conn: &mut Connection,
    upstream: &dyn Upstream,
    lists: &ChunkLists,
    servers: &mut Servers,
    (app, depot, gid): (u32, u32, u64),
    keep: bool,
) -> Result<Vec<Chunk>, String> {
    if let Some(c) = lists.kept(depot, gid) {
        return Ok(c);
    }
    let list = servers.get(conn)?;
    let chunks = fetch(conn, upstream, list, (app, depot, gid))?.chunks();
    if keep {
        lists.put(depot, gid, &chunks);
    }
    Ok(chunks)
}

/// `GET http://host/target`, the body of a 200 (at most a manifest's size).
fn get(upstream: &dyn Upstream, host: &str, target: &str) -> Result<Vec<u8>, String> {
    let req = UpstreamRequest { method: "GET", host, port: None, target, headers: &[("User-Agent", USER_AGENT)] };
    let mut r = upstream.fetch(&req).map_err(|e| e.to_string())?;
    if r.head().status != 200 {
        return Err(format!("status {}", r.head().status));
    }
    let mut body = Vec::new();
    let mut buf = vec![0u8; 64 << 10];
    loop {
        let n = r.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(body);
        }
        if body.len() + n > MAX_MANIFEST {
            return Err("a manifest over the limit".into());
        }
        body.extend_from_slice(&buf[..n]);
    }
}
