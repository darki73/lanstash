//! An Epic account's library, as Epic's launcher service and catalog answer
//! (**verified** on their answers: a library read whole, every game named).
//! The launcher service lists the account's assets
//! ([`ASSETS_PATH`], [`parse_assets`]): each an app (its name is what its
//! downloads are filed under), its catalog item and namespace, and its build.
//! The catalog describes an item ([`items_path`], [`parse_items`]): its title,
//! whether it is a game (a game or an application, not a DLC nor an engine
//! asset), and its images, each a URL on Epic's CDN ([`image`],
//! [`image_source`]). [`render_catalog`] gives the web UI's catalog view, in the
//! shape the other platforms' have.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use lanstash_domain::json::{Value, parse, string};
use lanstash_domain::service::{Profile, Route};
use lanstash_ports::art::ArtKind;

use crate::profile::Epic;
use lanstash_ports::upstream::Source;

/// Epic's launcher service.
pub const LAUNCHER_HOST: &str = "launcher-public-service-prod06.ol.epicgames.com";
/// Where an account's assets for Windows are listed.
pub const ASSETS_PATH: &str = "/launcher/api/public/assets/Windows?label=Live";
/// Epic's catalog.
pub const CATALOG_HOST: &str = "catalog-public-service-prod06.ol.epicgames.com";
/// Epic's library service: everything an account owns, page by page.
pub const LIBRARY_HOST: &str = "library-service.live.use1a.on.epicgames.com";

/// Where a page of an account's library is (the first without a cursor).
#[must_use]
pub fn records_path(cursor: Option<&str>) -> String {
    let base = "/library/api/public/items?includeMetadata=true";
    match cursor {
        Some(c) => format!("{base}&cursor={}", crate::auth::percent(c)),
        None => base.to_string(),
    }
}

/// A page of the library service's answer: its apps (as assets without a build) and the next
/// page's cursor, if there is one.
///
/// # Errors
/// Not a page of records (an error answer says its message).
pub fn parse_records(body: &str) -> Result<(Vec<Asset>, Option<String>), String> {
    let v = parse(body).map_err(|_| String::from("Epic's library is not JSON"))?;
    let Some(records) = v.get("records") else {
        let message = v.get("errorMessage").and_then(Value::as_str).unwrap_or("not a page of the library");
        return Err(format!("Epic: {message}"));
    };
    let apps = records
        .as_array()
        .iter()
        .map(|r| Asset {
            app_name: text(r, "appName"),
            namespace: text(r, "namespace"),
            catalog_item_id: text(r, "catalogItemId"),
            build_version: String::new(),
        })
        .filter(|a| safe(&a.app_name) && safe(&a.namespace) && safe(&a.catalog_item_id))
        .collect();
    let next =
        v.get("responseMetadata").and_then(|m| m.get("nextCursor")).and_then(Value::as_str).map(ToString::to_string);
    Ok((apps, next.filter(|c| !c.is_empty())))
}

/// One asset an account owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// What its downloads are filed under (`/Builds/Org/<org>/<app>/...`).
    pub app_name: String,
    pub namespace: String,
    pub catalog_item_id: String,
    pub build_version: String,
}

/// A catalog item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub title: String,
    /// A game or an application of its own (not a DLC, not an engine asset).
    pub game: bool,
    /// Its images: type (`DieselGameBox`, `DieselGameBoxTall`, ...) and URL.
    pub images: Vec<(String, String)>,
}

/// Whether `s` is safe in a request path: letters, digits, `-`, `_`, `.`.
fn safe(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The launcher service's list of an account's assets (those with an app, a namespace and a catalog item).
///
/// # Errors
/// Not a list (an error answer says its message).
pub fn parse_assets(body: &str) -> Result<Vec<Asset>, String> {
    let v = parse(body).map_err(|_| String::from("Epic's list of assets is not JSON"))?;
    if let Value::Array(list) = &v {
        return Ok(list
            .iter()
            .map(|a| Asset {
                app_name: text(a, "appName"),
                namespace: text(a, "namespace"),
                catalog_item_id: text(a, "catalogItemId"),
                build_version: text(a, "buildVersion"),
            })
            .filter(|a| safe(&a.app_name) && safe(&a.namespace) && safe(&a.catalog_item_id))
            .collect());
    }
    let message = v.get("errorMessage").and_then(Value::as_str).unwrap_or("not a list of assets");
    Err(format!("Epic: {message}"))
}

/// Where the launcher service says which manifests an app's current build has (for Windows).
#[must_use]
pub fn manifest_path(a: &Asset) -> Option<String> {
    (safe(&a.namespace) && safe(&a.catalog_item_id) && safe(&a.app_name)).then(|| {
        format!(
            "/launcher/api/public/assets/v2/platform/Windows/namespace/{}/catalogItem/{}/app/{}/label/Live",
            a.namespace, a.catalog_item_id, a.app_name
        )
    })
}

/// The directories an app's downloads are filed under, from that answer: each manifest URL's
/// game, as the cache files it ([`crate::profile::Epic`]), once each.
#[must_use]
pub fn parse_manifest_dirs(body: &str) -> Vec<String> {
    let Ok(v) = parse(body) else { return Vec::new() };
    let mut dirs: Vec<String> = Vec::new();
    for element in v.get("elements").map_or(&[][..], Value::as_array) {
        for m in element.get("manifests").map_or(&[][..], Value::as_array) {
            let Some(uri) = m.get("uri").and_then(Value::as_str) else { continue };
            let path = uri.split_once("://").map_or(uri, |(_, rest)| rest.find('/').map_or("", |i| &rest[i..]));
            if let Route::Object { game, .. } = Epic.route(path)
                && safe(&game)
                && !dirs.contains(&game)
            {
                dirs.push(game);
            }
        }
    }
    dirs
}

/// Where one of a build's manifests is: its CDN host, its path with the query the answer gives
/// (a signature, on some CDNs), and its directory (where the chunks' paths start).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestUrl {
    pub host: String,
    pub target: String,
    pub dir: String,
}

/// Each manifest URL of that answer (the build's, on each CDN), in its order.
#[must_use]
pub fn parse_manifest_urls(body: &str) -> Vec<ManifestUrl> {
    let Ok(v) = parse(body) else { return Vec::new() };
    let mut out = Vec::new();
    for element in v.get("elements").map_or(&[][..], Value::as_array) {
        for m in element.get("manifests").map_or(&[][..], Value::as_array) {
            let Some(uri) = m.get("uri").and_then(Value::as_str) else { continue };
            let Some((host, path)) = uri.split_once("://").and_then(|(_, rest)| rest.split_once('/')) else { continue };
            let path = format!("/{}", path.split('?').next().unwrap_or(path));
            let query: Vec<String> = m
                .get("queryParams")
                .map_or(&[][..], Value::as_array)
                .iter()
                .filter_map(|q| {
                    let (n, v) = (q.get("name")?.as_str()?, q.get("value")?.as_str()?);
                    Some(format!("{}={}", crate::auth::percent(n), crate::auth::percent(v)))
                })
                .collect();
            let target = if query.is_empty() { path.clone() } else { format!("{path}?{}", query.join("&")) };
            let dir = path.rsplit_once('/').map_or_else(|| String::from("/"), |(d, _)| format!("{d}/"));
            let ok = !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
            if ok {
                out.push(ManifestUrl { host: host.to_string(), target, dir });
            }
        }
    }
    out
}

/// Where the catalog describes item `id` of `namespace` (with its DLC and main game details).
#[must_use]
pub fn items_path(namespace: &str, id: &str) -> Option<String> {
    (safe(namespace) && safe(id)).then(|| {
        format!(
            "/catalog/api/shared/namespace/{namespace}/bulk/items?id={id}&includeDLCDetails=true&includeMainGameDetails=true&country=US&locale=en"
        )
    })
}

/// The catalog's answer: each item by its id.
///
/// # Errors
/// Not an object of items.
pub fn parse_items(body: &str) -> Result<Vec<(String, Item)>, String> {
    let v = parse(body).map_err(|_| String::from("Epic's catalog answer is not JSON"))?;
    if let Some(message) = v.get("errorMessage").and_then(Value::as_str) {
        return Err(format!("Epic: {message}"));
    }
    Ok(v.members()
        .iter()
        .map(|(id, item)| {
            let paths: Vec<&str> = item
                .get("categories")
                .map_or(&[][..], Value::as_array)
                .iter()
                .filter_map(|c| c.get("path")?.as_str())
                .collect();
            let main = item.get("mainGameItem").is_some_and(|m| !matches!(m, Value::Null));
            let game = !main && paths.iter().any(|p| *p == "games" || *p == "applications");
            let images = item
                .get("keyImages")
                .map_or(&[][..], Value::as_array)
                .iter()
                .filter_map(|k| Some((k.get("type")?.as_str()?.to_string(), k.get("url")?.as_str()?.to_string())))
                .collect();
            (id.clone(), Item { title: text(item, "title"), game, images })
        })
        .collect())
}

/// The URL of an item's image of `kind`: a wide one for the header and background, the logo,
/// the thumbnail for the icon; the first of their types it has.
#[must_use]
pub fn image(item: &Item, kind: ArtKind) -> Option<&str> {
    let types: &[&str] = match kind {
        ArtKind::Header => &["DieselGameBox", "OfferImageWide", "DieselStoreFrontWide"],
        ArtKind::Background => &["DieselGameBox", "OfferImageWide", "DieselStoreFrontWide"],
        ArtKind::Logo => &["DieselGameBoxLogo"],
        ArtKind::Icon => &["Thumbnail", "DieselGameBoxTall"],
    };
    types.iter().find_map(|t| item.images.iter().find(|(k, _)| k == t).map(|(_, url)| url.as_str()))
}

/// Where an image URL is fetched from: its host and path, over plain HTTP like the CDNs.
#[must_use]
pub fn image_source(url: &str) -> Option<Source> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let ok = !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
    (ok && host.ends_with(".epicgames.com")).then(|| Source {
        host: host.to_string(),
        port: None,
        target: format!("/{path}"),
    })
}

/// One game of the catalog view: its app, its title and its owners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game<'a> {
    pub asset: &'a Asset,
    pub title: &'a str,
    pub owners: Vec<String>,
    /// Bytes an install of its current build downloads, once worked out.
    pub download: Option<u64>,
}

/// The catalog view (`GET /api/v1/catalog/epicgames`): each game by title, its app name as code
/// and game, its build as version, its download size once worked out, and its owners.
#[must_use]
pub fn render_catalog(games: &[Game<'_>]) -> String {
    let mut sorted: Vec<&Game<'_>> = games.iter().collect();
    sorted.sort_by_key(|g| g.title.to_lowercase());
    let products: Vec<String> = sorted
        .iter()
        .map(|g| {
            let owners: Vec<String> = g.owners.iter().map(|o| string(o)).collect();
            let download = g.download.filter(|n| *n > 0).map_or_else(String::new, |n| format!(",\"download\":{n}"));
            format!(
                "{{\"code\":{app},\"name\":{},\"game\":{app},\"version\":{},\"known\":true{download},\"owners\":[{}]}}",
                string(g.title),
                string(&g.asset.build_version),
                owners.join(","),
                app = string(&g.asset.app_name),
            )
        })
        .collect();
    format!("{{\"products\":[{}]}}\n", products.join(","))
}

/// The view while there is no library: no games, and why.
#[must_use]
pub fn render_none(why: &str) -> String {
    format!("{{\"products\":[],\"error\":{}}}\n", string(why))
}
