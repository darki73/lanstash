//! Reading what a library is made of from Steam, through a logged-on
//! connection: the account's licenses, their packages (which apps and depots
//! each grants), then each granted app's access token and info, in batches.

use std::collections::BTreeSet;

use lanstash_steam_domain::cm::pics::{self, AccessTokens, Product, ProductInfo};
use lanstash_steam_domain::kv::{Kv, parse_binary};

use crate::connection::Connection;
use crate::raw::Raw;

/// Apps asked about in one request.
const BATCH: usize = 200;

/// Product info for `packages` and `apps` (with their tokens), all parts of the answer.
fn product_info(conn: &mut Connection, packages: &[(u32, u64)], apps: &[(u32, u64)]) -> Result<ProductInfo, String> {
    let job = conn.request(pics::emsg::PICS_PRODUCT_INFO_REQUEST, pics::product_info_request(packages, apps, false))?;
    let mut all = ProductInfo::default();
    loop {
        let m = conn.reply(job, pics::emsg::PICS_PRODUCT_INFO_RESPONSE)?;
        let p = ProductInfo::decode(&m.body).map_err(|e| format!("product info: {e:?}"))?;
        all.apps.extend(p.apps);
        all.packages.extend(p.packages);
        if !p.pending {
            return Ok(all);
        }
    }
}

/// The ids listed under `key` in a package's KeyValues (after its 4-byte prefix).
fn package_ids(p: &Product, key: &str) -> Vec<u32> {
    let kv = parse_binary(p.buffer.get(4..).unwrap_or_default()).unwrap_or(Kv::Map(Vec::new()));
    let root = kv.entries().first().map(|(_, v)| v.clone()).unwrap_or(Kv::Map(Vec::new()));
    root.get(key).map_or_else(Vec::new, |l| l.entries().iter().filter_map(|(_, v)| v.int()).map(|v| v as u32).collect())
}

/// Reads Steam's answers a library is made of: the depots the licenses' packages list
/// (`owned`), and the info of every app they list (`apps`).
///
/// # Errors
/// The connection, or an answer that does not parse.
pub fn read_raw(conn: &mut Connection) -> Result<Raw, String> {
    let licenses = conn.licenses()?;
    let packages: Vec<(u32, u64)> = licenses.iter().map(|l| (l.package_id, l.access_token)).collect();
    let info = product_info(conn, &packages, &[])?;
    let ids: BTreeSet<u32> = info.packages.iter().flat_map(|p| package_ids(p, "appids")).collect();
    let owned: BTreeSet<u32> = info.packages.iter().flat_map(|p| package_ids(p, "depotids")).collect();
    let ids: Vec<u32> = ids.into_iter().collect();
    let mut apps = Vec::new();
    for batch in ids.chunks(BATCH) {
        let job = conn.request(pics::emsg::PICS_ACCESS_TOKEN_REQUEST, pics::access_token_request(&[], batch))?;
        let tokens = AccessTokens::decode(&conn.reply(job, pics::emsg::PICS_ACCESS_TOKEN_RESPONSE)?.body)
            .map_err(|e| format!("access tokens: {e:?}"))?;
        let with: Vec<(u32, u64)> =
            batch.iter().map(|a| (*a, tokens.apps.iter().find(|(id, _)| id == a).map_or(0, |t| t.1))).collect();
        apps.extend(product_info(conn, &[], &with)?.apps.into_iter().map(|p| (p.id, p.buffer)));
    }
    Ok(Raw { owned: owned.into_iter().collect(), apps })
}
