//! The Epic library as kept between runs ([`Saved`]): when it was read, each
//! account's assets, each catalog item, and each download directory's app, as
//! one JSON document ([`encode`], [`decode`]).

use std::collections::BTreeMap;

use lanstash_domain::json::{Value, parse, string};
use lanstash_epic::library::{Asset, Item};

/// What is kept of the library.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    /// Each account's assets (by account id), in the order they signed in.
    pub assets: Vec<(String, Vec<Asset>)>,
    /// Catalog items by id.
    pub items: BTreeMap<String, Item>,
    /// The directory an app's downloads are filed under, to the app.
    pub dirs: BTreeMap<String, String>,
    pub at_ms: u64,
}

/// The library as JSON.
#[must_use]
pub fn encode(s: &Saved) -> String {
    let assets: Vec<String> = s
        .assets
        .iter()
        .map(|(account, list)| {
            let list: Vec<String> = list
                .iter()
                .map(|a| {
                    format!(
                        "[{},{},{},{}]",
                        string(&a.app_name),
                        string(&a.namespace),
                        string(&a.catalog_item_id),
                        string(&a.build_version)
                    )
                })
                .collect();
            format!("[{},[{}]]", string(account), list.join(","))
        })
        .collect();
    let items: Vec<String> = s
        .items
        .iter()
        .map(|(id, i)| {
            let images: Vec<String> = i.images.iter().map(|(t, u)| format!("[{},{}]", string(t), string(u))).collect();
            format!("[{},{},{},[{}]]", string(id), string(&i.title), i.game, images.join(","))
        })
        .collect();
    let dirs: Vec<String> = s.dirs.iter().map(|(d, a)| format!("[{},{}]", string(d), string(a))).collect();
    format!(
        "{{\"at\":{},\"assets\":[{}],\"items\":[{}],\"dirs\":[{}]}}",
        s.at_ms,
        assets.join(","),
        items.join(","),
        dirs.join(",")
    )
}

fn texts(v: &Value) -> Vec<String> {
    v.as_array().iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect()
}

/// A kept library (one without directories: none known); none when it is not one.
#[must_use]
pub fn decode(text: &str) -> Option<Saved> {
    let v = parse(text).ok()?;
    let at_ms = match v.get("at")? {
        Value::Number(n) => *n as u64,
        _ => return None,
    };
    let assets = v
        .get("assets")?
        .as_array()
        .iter()
        .filter_map(|pair| {
            let pair = pair.as_array();
            let account = pair.first()?.as_str()?.to_string();
            let list = pair
                .get(1)?
                .as_array()
                .iter()
                .filter_map(|a| {
                    let t = texts(a);
                    Some(Asset {
                        app_name: t.first()?.clone(),
                        namespace: t.get(1)?.clone(),
                        catalog_item_id: t.get(2)?.clone(),
                        build_version: t.get(3)?.clone(),
                    })
                })
                .collect();
            Some((account, list))
        })
        .collect();
    let items = v
        .get("items")?
        .as_array()
        .iter()
        .filter_map(|i| {
            let i = i.as_array();
            let images = i
                .get(3)?
                .as_array()
                .iter()
                .map(texts)
                .filter_map(|p| Some((p.first()?.clone(), p.get(1)?.clone())))
                .collect();
            let item =
                Item { title: i.get(1)?.as_str()?.to_string(), game: matches!(i.get(2)?, Value::Bool(true)), images };
            Some((i.first()?.as_str()?.to_string(), item))
        })
        .collect();
    let dirs = v
        .get("dirs")
        .map_or(&[][..], Value::as_array)
        .iter()
        .map(texts)
        .filter_map(|p| Some((p.first()?.clone(), p.get(1)?.clone())))
        .collect();
    Some(Saved { assets, items, dirs, at_ms })
}
