//! Battle.net's product catalog as an index from product code to name and art
//! (the documents are described in [`lanstash_battlenet_domain::catalog`]). Built on first use through the
//! [`Upstream`] port: the versions and CDN tables from the patch service,
//! then the build config, the root and every base fragment from the CDN
//! (about 50 small requests). Checked again after
//! [`CatalogOptions::refresh_ms`]; rebuilt only when the catalog build
//! changed. A failed build is retried after [`CatalogOptions::retry_ms`],
//! and an index already built is kept meanwhile.
//!
//! A game of lanstash's is a CDN path (`Hero-Live-a`), not a product code
//! (`hero`): the path is tried as a code (`osi`, `drtl`), then the code the
//! patch service names for it ([`CodeOf`], given once composed).

use std::sync::{Arc, Mutex, PoisonError, Weak};

use lanstash_battlenet_domain::catalog::{Product, base_fragments, current_build, global_types, product_with};
use lanstash_battlenet_domain::tact::{PATCH_HOST, PATCH_PORT, cdn, config_value, target};
use lanstash_domain::json;

use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::upstream::Source;

use lanstash_app::origin::{self, OriginError};
use lanstash_ports::clock::Clock;
use lanstash_ports::upstream::Upstream;

/// How the catalog is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogOptions {
    /// Time between checks for a newer catalog.
    pub refresh_ms: u64,
    /// Time before trying again after a failure.
    pub retry_ms: u64,
    /// Largest catalog document accepted.
    pub max_document: usize,
    /// The CDN region to fetch from.
    pub region: String,
}

impl Default for CatalogOptions {
    fn default() -> Self {
        Self { refresh_ms: 6 * 3_600_000, retry_ms: 60_000, max_document: 8 << 20, region: "us".into() }
    }
}

struct Index {
    build: String,
    host: String,
    path: String,
    products: Vec<Product>,
}

#[derive(Default)]
struct State {
    index: Option<Arc<Index>>,
    checked_ms: Option<u64>,
    /// The last build failure, until a build succeeds.
    failed: Option<String>,
}

pub struct BlizzardCatalog {
    upstream: Arc<dyn Upstream>,
    clock: Arc<dyn Clock>,
    opts: CatalogOptions,
    state: Mutex<State>,
    /// Where a CDN path'"'"'s product code is found (set once composed).
    codes: Mutex<Option<Weak<dyn CodeOf>>>,
}

impl std::fmt::Debug for BlizzardCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlizzardCatalog").field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl BlizzardCatalog {
    #[must_use]
    pub fn new(upstream: Arc<dyn Upstream>, clock: Arc<dyn Clock>, opts: CatalogOptions) -> Self {
        Self { upstream, clock, opts, state: Mutex::new(State::default()), codes: Mutex::new(None) }
    }

    /// Where the `kind` image of game `game` (a CDN path) is.
    ///
    /// # Errors
    /// [`ArtError::Missing`] when the catalog has no such product or image;
    /// [`ArtError::Upstream`] when there is no catalog yet.
    pub fn locate(&self, game: &str, kind: ArtKind) -> Result<Source, ArtError> {
        let index = self.index()?;
        let p = self.find(&index.products, game).ok_or(ArtError::Missing)?;
        // An install's own images where `game` is one of the product's install codes.
        let install = p.installs.iter().find(|i| i.code == game);
        let hash = match kind {
            ArtKind::Header => install.and_then(|i| i.key_art.as_deref()).or(p.key_art.as_deref()),
            ArtKind::Icon => p.icon.as_deref(),
            ArtKind::Background => install.and_then(|i| i.background.as_deref()).or(p.background.as_deref()),
            ArtKind::Logo => install.and_then(|i| i.logo.as_deref()).or(p.logo.as_deref()),
        }
        .ok_or(ArtError::Missing)?;
        let target = target(&index.path, "data", hash).ok_or(ArtError::Missing)?;
        Ok(Source { host: index.host.clone(), port: None, target })
    }

    /// The catalog's name of game `game`, if the catalog is built (never waits for it).
    #[must_use]
    pub fn product_name(&self, game: &str) -> Option<String> {
        // The index out of the lock first: finding a path's product may ask the
        // products for its code, which may list this catalog (and take the lock).
        let index = self.state.try_lock().ok()?.index.clone()?;
        self.find(&index.products, game)?.name.clone()
    }

    /// Every product of the catalog (built first when due).
    ///
    /// # Errors
    /// [`ArtError::Upstream`] when there is no catalog yet.
    pub fn products(&self) -> Result<Vec<Product>, ArtError> {
        Ok(self.index()?.products.clone())
    }

    /// The index, built or refreshed first when due.
    fn index(&self) -> Result<Arc<Index>, ArtError> {
        let mut s = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let now = self.clock.now_ms();
        let wait = if s.failed.is_some() { self.opts.retry_ms } else { self.opts.refresh_ms };
        if s.checked_ms.is_none_or(|at| now.saturating_sub(at) >= wait) {
            s.checked_ms = Some(now);
            match self.build(s.index.as_deref()) {
                Ok(Some(index)) => {
                    s.index = Some(Arc::new(index));
                    s.failed = None;
                }
                Ok(None) => s.failed = None,
                Err(e) => s.failed = Some(e),
            }
        }
        match (&s.index, &s.failed) {
            (Some(i), _) => Ok(Arc::clone(i)),
            (None, e) => Err(ArtError::Upstream(format!("Battle.net catalog: {}", e.as_deref().unwrap_or("none")))),
        }
    }

    fn text(&self, src: &Source) -> Result<String, String> {
        let body = origin::get(self.upstream.as_ref(), src, self.opts.max_document).map_err(|e| match e {
            OriginError::Failed(m) => m,
            OriginError::Missing => format!("{}{}: not found", src.host, src.target),
        })?;
        String::from_utf8(body).map_err(|_| format!("{}{}: not UTF-8", src.host, src.target))
    }

    fn patch(&self, target: &str) -> Result<String, String> {
        self.text(&Source { host: PATCH_HOST.into(), port: Some(PATCH_PORT), target: target.into() })
    }

    fn cdn_doc(&self, host: &str, path: &str, kind: &str, hash: &str) -> Result<String, String> {
        let target = target(path, kind, hash).ok_or_else(|| format!("bad hash {hash:?}"))?;
        self.text(&Source { host: host.into(), port: None, target })
    }

    /// A new index, or `None` when `old` is still the current build.
    fn build(&self, old: Option<&Index>) -> Result<Option<Index>, String> {
        let build = current_build(&self.patch("/catalogs/versions")?).ok_or("no catalog in /catalogs/versions")?;
        if old.is_some_and(|o| o.build == build) {
            return Ok(None);
        }
        let (host, path) = cdn(&self.patch("/catalogs/cdns")?, &self.opts.region).ok_or("no CDN in /catalogs/cdns")?;
        let config = self.cdn_doc(&host, &path, "config", &build)?;
        let root_hash = config_value(&config, "root").ok_or("no root in the catalog config")?;
        let root = json::parse(&self.cdn_doc(&host, &path, "data", root_hash)?)
            .map_err(|e| format!("catalog root: {} at {}", e.what, e.at))?;
        let shared = global_types(&root);
        let mut products = Vec::new();
        for (_, hash) in base_fragments(&root) {
            // One bad fragment costs its product, not the catalog.
            if let Some(p) = self
                .cdn_doc(&host, &path, "data", &hash)
                .ok()
                .and_then(|t| json::parse(&t).ok())
                .and_then(|f| product_with(&f, &shared))
            {
                products.push(p);
            }
        }
        if products.is_empty() {
            return Err("the catalog lists no products".into());
        }
        Ok(Some(Index { build, host, path, products }))
    }
}

/// Which product code a Battle.net CDN path belongs to, as the patch service
/// says ([`crate::products::BlizzardProducts`] knows).
pub trait CodeOf: Send + Sync {
    fn code_of(&self, game: &str) -> Option<String>;
}

impl BlizzardCatalog {
    /// Where to ask which product code a CDN path is (once, after composing;
    /// held weakly, as it is built from this catalog).
    pub fn set_codes(&self, codes: Weak<dyn CodeOf>) {
        *self.codes.lock().unwrap_or_else(PoisonError::into_inner) = Some(codes);
    }

    /// The product of a game: the path as a code, then the code the patch service names for it.
    fn find<'a>(&self, products: &'a [Product], game: &str) -> Option<&'a Product> {
        let by = |code: &str| products.iter().find(|p| p.codes.iter().any(|c| c == code));
        if let Some(p) = by(game) {
            return Some(p);
        }
        let codes = self.codes.lock().unwrap_or_else(PoisonError::into_inner).as_ref()?.upgrade()?;
        by(&codes.code_of(game)?)
    }
}

/// Battle.net art: the images the catalog names.
impl ArtSource for BlizzardCatalog {
    fn locate(&self, id: &str, kind: ArtKind) -> Result<Source, ArtError> {
        BlizzardCatalog::locate(self, id, kind)
    }
}

/// Names from the catalog, for Battle.net games, once it is built.
impl lanstash_ports::names::GameNames for BlizzardCatalog {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        (service == "blizzard").then(|| self.product_name(game)).flatten()
    }
}
