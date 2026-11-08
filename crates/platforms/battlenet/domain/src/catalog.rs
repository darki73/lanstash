//! Battle.net's product catalog: the launcher's own
//! list of products, with their names and art. All of it is plain HTTP and
//! plain text:
//!
//! 1. `us.patch.battle.net:1119/catalogs/versions` and `/catalogs/cdns`
//!    ([`crate::tact`] tables). The current catalog is the row with the highest `VersionsName` (the
//!    build number; `BuildId` is not monotonic: PUB-29 has 422, PUB-30 407).
//! 2. `/<path>/config/<ab>/<cd>/<hash>`: the build config, `key = value` lines;
//!    `root` names the root document.
//! 3. `/<path>/data/<ab>/<cd>/<hash>`: the root (JSON) lists `fragments`; each
//!    fragment (JSON) describes products: `installs.*.tact_product` (product
//!    codes), `products[0].base` (keys of the name and the images),
//!    `strings.default` and `files.default` (key → `{hash, name}`).
//!
//! Every file is named by the MD5 of its content. Fragments with `requires`
//! are regional or licence overrides, and those with `decryption_key_id` are
//! encrypted (unreleased products); both are skipped.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::tact::{bpsv, column, is_hash};
use lanstash_domain::json::Value;

/// The build config hash of the current catalog, from `/catalogs/versions`.
#[must_use]
pub fn current_build(versions: &str) -> Option<String> {
    bpsv(versions)
        .iter()
        .filter_map(|r| {
            let n: u64 = column(r, "VersionsName")?.parse().ok()?;
            let h = column(r, "BuildConfig").filter(|h| is_hash(h))?;
            Some((n, h))
        })
        .max_by_key(|(n, _)| *n)
        .map(|(_, h)| h.to_owned())
}

/// The base fragments of a catalog root: `(name, hash)`.
#[must_use]
pub fn base_fragments(root: &Value) -> Vec<(String, String)> {
    root.get("fragments")
        .map(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter(|f| f.get("requires").is_none() && f.get("decryption_key_id").is_none())
        .filter_map(|f| {
            let name = f.get("name")?.as_str()?;
            let hash = f.get("hash")?.as_str().filter(|h| is_hash(h))?;
            Some((name.to_owned(), hash.to_owned()))
        })
        .collect()
}

/// One way a product is installed in the launcher: its product code (the
/// patch service's), and its name there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub code: String,
    pub name: Option<String>,
    /// Hashes of its images: the wide background behind its page in the
    /// launcher, its logo, its key art (a portrait poster); its own where its
    /// type or its product gives them, else its game's.
    pub background: Option<String>,
    pub logo: Option<String>,
    pub key_art: Option<String>,
}

impl Install {
    /// An install without images.
    #[must_use]
    pub fn new(code: impl Into<String>, name: Option<String>) -> Self {
        Self { code: code.into(), name, background: None, logo: None, key_art: None }
    }
}

/// What the catalog says about one product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// The product codes players install it by (the main one first; test,
    /// development and vendor builds left out): [`Product::installs`]' codes.
    pub codes: Vec<String>,
    /// Those installs, with their names (World of Warcraft: retail, Classic,
    /// Classic Era, Anniversary, Forever, ...).
    pub installs: Vec<Install>,
    pub name: Option<String>,
    /// Hash of its square icon (the largest there is).
    pub icon: Option<String>,
    /// Hash of its key art (a portrait poster, logo near the bottom).
    pub key_art: Option<String>,
    /// Hashes of the wide background behind its page in the launcher, and of its logo.
    pub background: Option<String>,
    pub logo: Option<String>,
}

/// Categories of install types players do not get: in development (PTR,
/// beta, alpha), vendors, events, tournaments, unknown.
const INTERNAL: &[&str] =
    &["cat_in_development", "cat_vendor", "cat_event", "cat_professional", "cat_tournament", "cat_unknown"];

/// An install type: its category, how its name is made, its name key.
#[derive(Debug, Clone)]
struct Kind {
    category: String,
    name_style: Option<String>,
    type_name: Option<String>,
}

fn kinds(doc: &Value) -> Vec<(String, Kind)> {
    doc.get("types")
        .and_then(|t| t.get("definitions"))
        .map(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|d| {
            let id = d.get("id")?.as_str()?.to_owned();
            let defaults = d.get("product_defaults");
            let text = |k: &str| defaults.and_then(|p| p.get(k)).and_then(Value::as_str).map(str::to_owned);
            let category = d.get("category").and_then(Value::as_str).unwrap_or("cat_unknown").to_owned();
            Some((id, Kind { category, name_style: text("name_style"), type_name: text("type_name") }))
        })
        .collect()
}

/// The install types every product shares, from the catalog root (`retail`,
/// `ptr`, `beta`, `vendor_1`, ...); a product's own fragment adds its own.
#[derive(Debug, Clone, Default)]
pub struct Types(Vec<(String, Kind)>);

/// The root's install types.
#[must_use]
pub fn global_types(root: &Value) -> Types {
    Types(kinds(root))
}

/// The product a fragment describes, if it describes one, with no shared
/// install types known: see [`product_with`].
#[must_use]
pub fn product(fragment: &Value) -> Option<Product> {
    product_with(fragment, &Types::default())
}

/// The product a fragment describes, if players can install it some way.
///
/// The fragment's main product (the one not pointing at another by
/// `real_product_id`) gives the name and art. Each install's type is the one
/// a product declares for it (`base.types.<type>.uid` names the install, in
/// any case: `hs_beta` is Hearthstone's retail, `aqua_retail_rc` Avowed's
/// `rc`, `D1H` Diablo's Hellfire); an install no product declares is its key
/// among the fragment's types, else its key without the main install's prefix
/// among the shared ones (`wow_ptr` is `ptr`), and the main install (the
/// shortest key) is retail. A type is found among the fragment's own types,
/// then the shared ones. An install is left out when its type is internal
/// ([`INTERNAL`] categories), when its declared type is disabled in every
/// region, when it is `deprecated`, or, in a fragment that gives installs an
/// `auto_update_policy`, when it has none (development and vendor builds).
/// On the real catalog, these rules give World of Warcraft's retail, Classic,
/// Classic Era, Titan, Anniversary and Forever installs, and the declared
/// types leave out the development and release-candidate builds of Avowed,
/// Doom: The Dark Ages, Sea of Thieves and Warcraft Rumble, and the
/// vendor-only Kelp and Spot.
///
/// An install's name (but the main one's): its name in the launcher's version
/// picker where its product declares one (`selector_display_name`: "Burning
/// Crusade Anniversary", "Hellfire"), else the label of the first of the game
/// modes it launches (`game_packages` by `rank`: "Classic Era"); else its type's name when the type names
/// itself (`name_style` `default`: "WoW: Forever"), else the name of the product
/// whose `default_product_type` is its type ("World of Warcraft Classic"),
/// else the main product's name with its type's name after it ("World of Warcraft
/// Classic" for Classic Era), else the main product's name.
#[must_use]
pub fn product_with(fragment: &Value, shared: &Types) -> Option<Product> {
    let products = fragment.get("products").map(Value::as_array).unwrap_or_default();
    let main = products
        .iter()
        .find(|p| p.get("base").is_some_and(|b| b.get("real_product_id").is_none()))
        .or_else(|| products.first())?;
    let base = main.get("base")?;
    let strings = fragment.get("strings").and_then(|s| s.get("default"));
    let say = |k: &str| strings.and_then(|s| s.get(k)).and_then(Value::as_str).map(str::to_owned);
    let name_of = |p: &Value| p.get("base")?.get("name")?.as_str().and_then(say);
    let name = name_of(main);
    let own = kinds(fragment);
    let raw: Vec<(&str, &Value)> = fragment
        .get("installs")
        .map(Value::members)
        .unwrap_or_default()
        .iter()
        .filter_map(|(k, i)| i.get("tact_product")?.as_str().map(|_| (k.as_str(), i)))
        .collect();
    let main_key = raw.iter().map(|(k, _)| *k).min_by_key(|k| (k.len(), *k))?;
    let policies = raw.iter().any(|(_, i)| i.get("auto_update_policy").is_some());
    // (install key in lower case, type, disabled in every region, its name in the launcher's
    // version picker) as the products declare them.
    let declared: Vec<(String, String, bool, Option<String>)> = products
        .iter()
        .filter_map(|p| p.get("base")?.get("types"))
        .flat_map(Value::members)
        .filter_map(|(id, t)| {
            let uid = t.get("uid")?.as_str()?.to_ascii_lowercase();
            let flags = t.get("region_permission_flags");
            let off = flags.and_then(|f| f.get("default")).and_then(Value::as_str) == Some("disabled")
                && flags.and_then(|f| f.get("overrides")).is_none();
            // The version picker's name; without one, the label of its first game mode (World of
            // Warcraft Classic Era's install launches Classic Era, Hardcore, Season of Discovery).
            let first_mode = || {
                let modes = t.get("game_packages").map(Value::as_array).unwrap_or_default();
                let rank = |g: &Value| match g.get("rank") {
                    Some(Value::Number(n)) => *n,
                    _ => f64::MAX,
                };
                modes.iter().min_by(|a, b| rank(a).total_cmp(&rank(b)))?.get("label")?.as_str().and_then(say)
            };
            let picker = t.get("selector_display_name").and_then(Value::as_str).and_then(say).or_else(first_mode);
            Some((uid, id.clone(), off, picker))
        })
        .collect();
    let declared_of = |key: &str| declared.iter().find(|(uid, ..)| uid.eq_ignore_ascii_case(key));
    let type_of = |id: &str| own.iter().chain(shared.0.iter()).find(|(i, _)| i == id).map(|(_, k)| k);
    let kind_of = |key: &str| -> (String, Option<&Kind>) {
        if let Some((_, id, ..)) = declared_of(key) {
            return (id.clone(), type_of(id));
        }
        if let Some((id, k)) = own.iter().find(|(id, _)| id == key) {
            return (id.clone(), Some(k));
        }
        let id = if key == main_key {
            "retail".to_owned()
        } else {
            key.strip_prefix(main_key).and_then(|r| r.strip_prefix('_')).unwrap_or(key).to_owned()
        };
        let k = shared.0.iter().find(|(i, _)| *i == id).map(|(_, k)| k);
        (id, k)
    };
    let files = fragment.get("files").and_then(|f| f.get("default"));
    let file = |key: &str| files?.get(key)?.get("hash")?.as_str().filter(|h| is_hash(h)).map(str::to_owned);
    // The type a product declares for an install: its images are the install's own.
    let type_entry = |key: &str| {
        products
            .iter()
            .filter_map(|p| p.get("base")?.get("types"))
            .flat_map(Value::members)
            .map(|(_, t)| t)
            .find(|t| t.get("uid").and_then(Value::as_str).is_some_and(|u| u.eq_ignore_ascii_case(key)))
    };
    let image = |v: Option<&Value>, field: &str| v.and_then(|v| v.get(field)).and_then(Value::as_str).and_then(file);
    let mut installs: Vec<(bool, Install)> = Vec::new();
    for (key, inst) in &raw {
        let (id, kind) = kind_of(key);
        let main = id == "retail";
        let internal = kind.is_some_and(|k| INTERNAL.contains(&k.category.as_str()))
            || declared_of(key).is_some_and(|(_, _, off, _)| *off)
            || inst.get("deprecated").is_some_and(|d| matches!(d, Value::Bool(true)))
            || (policies && inst.get("auto_update_policy").is_none());
        if internal {
            continue;
        }
        let own_name = kind.filter(|k| k.name_style.as_deref() == Some("default") && !main);
        let by_type = products.iter().find(|p| {
            p.get("base").and_then(|b| b.get("default_product_type")).and_then(Value::as_str) == Some(id.as_str())
        });
        let suffixed = || {
            let suffix = kind.filter(|_| !main)?.type_name.as_deref().and_then(say)?;
            let main = name.as_deref()?;
            Some(if main.ends_with(&suffix) { main.to_owned() } else { format!("{main} {suffix}") })
        };
        let picker = declared_of(key).and_then(|(.., n)| n.clone()).filter(|_| !main);
        let install_name = picker
            .or_else(|| own_name.and_then(|k| k.type_name.as_deref().and_then(say)))
            .or_else(|| by_type.and_then(name_of))
            .or_else(suffixed)
            .or_else(|| name.clone());
        let code = inst.get("tact_product").and_then(Value::as_str).unwrap_or(key).to_owned();
        // Its images: its type's, else those of the product made for its type, else the game's.
        let own = by_type.and_then(|p| p.get("base"));
        let art = |field: &str| {
            image(type_entry(key), field).or_else(|| image(own, field)).or_else(|| image(Some(base), field))
        };
        let install = Install {
            code,
            name: install_name,
            background: art("background"),
            logo: art("logo"),
            key_art: image(own, "key_art").or_else(|| image(Some(base), "key_art")),
        };
        installs.push((main, install));
    }
    if installs.is_empty() {
        return None;
    }
    installs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.code.cmp(&b.1.code)));
    let installs: Vec<Install> = installs.into_iter().map(|(_, i)| i).collect();
    let key = |field: &str| base.get(field).and_then(Value::as_str);
    let medium = key("icon_medium");
    let icon = medium
        .and_then(|m| m.strip_suffix("_ICON_MEDIUM"))
        .and_then(|stem| file(&format!("{stem}_ICON_MASSIVE")))
        .or_else(|| medium.and_then(file));
    let key_art = key("key_art").and_then(file);
    let (background, logo) = (key("background").and_then(file), key("logo").and_then(file));
    let codes = installs.iter().map(|i| i.code.clone()).collect();
    Some(Product { codes, installs, name, icon, key_art, background, logo })
}
