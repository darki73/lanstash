//! Every Battle.net game there is, cached or not:
//! the launcher catalog's products ([`ProductList`], [`crate::catalog`])
//! with, per product code, the game directory its files are under on the CDN
//! (`/<code>/cdns`: `tpr/<game>`, [`game_of_path`]) and its current version in the region
//! (`/<code>/versions`), both from the patch service through the
//! [`Upstream`] port.
//!
//! Looking up ~45 products takes ~90 small requests, so [`BlizzardProducts::list`]
//! never waits for them: it answers with what is known and looks up the rest
//! on a thread of its own. Lookups are kept for [`ProductOptions::ttl_ms`]
//! (failures for [`ProductOptions::retry_ms`]). [`BlizzardProducts::code_of`]
//! maps a game directory back to its product, so a game not in the stash
//! yet can be prefilled.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lanstash_battlenet_domain::catalog::Product;
use lanstash_battlenet_domain::tact::{PATCH_HOST, PATCH_PORT, cdn, game_of_path, version};

use crate::catalog::CodeOf;
use lanstash_ports::upstream::Source;

use lanstash_app::origin::{self, OriginError};
use lanstash_ports::clock::Clock;
use lanstash_ports::upstream::Upstream;

/// The products there are (the launcher catalog in the app).
pub trait ProductList: Send + Sync {
    /// Every product.
    ///
    /// # Errors
    /// Why there is no list yet.
    fn products(&self) -> Result<Vec<Product>, String>;
}

impl ProductList for crate::catalog::BlizzardCatalog {
    fn products(&self) -> Result<Vec<Product>, String> {
        crate::catalog::BlizzardCatalog::products(self).map_err(|e| format!("{e:?}"))
    }
}

/// How products are looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductOptions {
    /// The region whose CDN path and version count.
    pub region: String,
    pub ttl_ms: u64,
    pub retry_ms: u64,
}

impl Default for ProductOptions {
    fn default() -> Self {
        Self { region: "us".into(), ttl_ms: 10 * 60_000, retry_ms: 60_000 }
    }
}

/// One product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProduct {
    /// Its product code (`s1`): one install of a catalog product (World of
    /// Warcraft lists retail, Classic, Classic Era, ..., each its own code).
    pub code: String,
    pub name: Option<String>,
    /// The name of the catalog product it is an install of ("World of Warcraft").
    pub family: Option<String>,
    /// Its directory on the CDN (`s1`, `Hero-Live-a`), once looked up; `None`
    /// also when the region has no CDN for it.
    pub game: Option<String>,
    /// Its current version in the region, once looked up.
    pub version: Option<String>,
    /// Whether it has been looked up (successfully or not).
    pub known: bool,
}

/// A product code's lookup: when, and what it found (its directory and version, if any).
type Looked = (u64, Result<Found, String>);

/// What a product code's lookup found: its directory and version, if any.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Found {
    game: Option<String>,
    version: Option<String>,
}

struct Inner {
    list: Arc<dyn ProductList>,
    upstream: Arc<dyn Upstream>,
    clock: Arc<dyn Clock>,
    opts: ProductOptions,
    /// By product code: when looked up, and what was found.
    found: Mutex<BTreeMap<String, Looked>>,
    looking: AtomicBool,
}

pub struct BlizzardProducts {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for BlizzardProducts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlizzardProducts").field("opts", &self.inner.opts).finish_non_exhaustive()
    }
}

/// Product codes looked up at once.
const LOOKUPS: usize = 8;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl BlizzardProducts {
    #[must_use]
    pub fn new(
        list: Arc<dyn ProductList>,
        upstream: Arc<dyn Upstream>,
        clock: Arc<dyn Clock>,
        opts: ProductOptions,
    ) -> Self {
        let inner =
            Inner { list, upstream, clock, opts, found: Mutex::new(BTreeMap::new()), looking: AtomicBool::new(false) };
        Self { inner: Arc::new(inner) }
    }

    /// Every product, with what is known of each; looks up the rest in the
    /// background.
    ///
    /// # Errors
    /// Why there is no product list yet.
    pub fn list(&self) -> Result<Vec<CatalogProduct>, String> {
        let products = self.inner.list.products()?;
        let now = self.inner.clock.now_ms();
        let found = lock(&self.inner.found);
        let mut stale = false;
        let out = products
            .iter()
            .flat_map(|p| p.installs.iter().map(move |i| (p, i)))
            .map(|(p, i)| {
                let got = found.get(&i.code);
                stale |= got.is_none_or(|(at, r)| self.inner.due(now, *at, r));
                let f = got.and_then(|(_, r)| r.as_ref().ok()).cloned().unwrap_or_default();
                CatalogProduct {
                    code: i.code.clone(),
                    name: i.name.clone().or_else(|| p.name.clone()),
                    family: p.name.clone(),
                    game: f.game,
                    version: f.version,
                    known: got.is_some(),
                }
            })
            .collect();
        drop(found);
        if stale && !self.inner.looking.swap(true, Ordering::SeqCst) {
            let inner = Arc::clone(&self.inner);
            let codes: Vec<String> = products.iter().flat_map(|p| p.codes.iter().cloned()).collect();
            let spawned =
                std::thread::Builder::new().name("lanstash-products".into()).spawn(move || inner.look_up(&codes));
            if spawned.is_err() {
                self.inner.looking.store(false, Ordering::SeqCst);
            }
        }
        Ok(out)
    }

    /// The product code whose files are under `game` on the CDN, as far as
    /// looked up; when not found, the lookups due are started (as
    /// [`BlizzardProducts::list`] does), so a later call finds it.
    #[must_use]
    pub fn code_of(&self, game: &str) -> Option<String> {
        self.codes_of(game).into_iter().next()
    }

    /// Every product code whose files are under `game` on the CDN (several
    /// products share one: World of Warcraft's retail and Classic flavours),
    /// a catalog product's main install first; as far as looked up, starting
    /// the lookups due when none is found.
    #[must_use]
    pub fn codes_of(&self, game: &str) -> Vec<String> {
        let mut codes: Vec<String> = lock(&self.inner.found)
            .iter()
            .filter(|(_, (_, r))| r.as_ref().is_ok_and(|f| f.game.as_deref() == Some(game)))
            .map(|(code, _)| code.clone())
            .collect();
        if codes.is_empty() {
            let _ = self.list();
            return codes;
        }
        let mains: Vec<String> =
            self.inner.list.products().unwrap_or_default().iter().filter_map(|p| p.codes.first().cloned()).collect();
        codes.sort_by_key(|c| !mains.contains(c));
        codes
    }
}

impl CodeOf for BlizzardProducts {
    fn code_of(&self, game: &str) -> Option<String> {
        BlizzardProducts::code_of(self, game)
    }
}

impl Inner {
    fn due(&self, now: u64, at: u64, r: &Result<Found, String>) -> bool {
        now.saturating_sub(at) >= if r.is_ok() { self.opts.ttl_ms } else { self.opts.retry_ms }
    }

    /// Looks up the codes due, [`LOOKUPS`] at a time (each is two small
    /// requests to the patch service, and the catalog lists well over a
    /// hundred installs).
    fn look_up(&self, codes: &[String]) {
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..LOOKUPS.min(codes.len().max(1)) {
                s.spawn(|| {
                    while let Some(code) = codes.get(next.fetch_add(1, Ordering::SeqCst)) {
                        let now = self.clock.now_ms();
                        let due = lock(&self.found).get(code).is_none_or(|(at, r)| self.due(now, *at, r));
                        if due {
                            let r = self.one(code);
                            lock(&self.found).insert(code.clone(), (now, r));
                        }
                    }
                });
            }
        });
        self.looking.store(false, Ordering::SeqCst);
    }

    fn table(&self, code: &str, table: &str) -> Result<Option<String>, String> {
        let src = Source { host: PATCH_HOST.into(), port: Some(PATCH_PORT), target: format!("/{code}/{table}") };
        match origin::get(self.upstream.as_ref(), &src, 1 << 20) {
            Ok(body) => Ok(Some(String::from_utf8_lossy(&body).into_owned())),
            Err(OriginError::Missing) => Ok(None),
            Err(OriginError::Failed(e)) => Err(e),
        }
    }

    fn one(&self, code: &str) -> Result<Found, String> {
        let game = self
            .table(code, "cdns")?
            .and_then(|t| cdn(&t, &self.opts.region))
            .and_then(|(_, path)| game_of_path(&path).map(str::to_owned));
        let version = self.table(code, "versions")?.and_then(|t| version(&t, &self.opts.region)).map(|v| v.name);
        Ok(Found { game, version })
    }
}
