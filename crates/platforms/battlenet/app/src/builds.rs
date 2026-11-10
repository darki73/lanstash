//! Whether the Battle.net games in the stash are on their current build.
//! For each Battle.net game directory (a CDN path),
//! its product code (the directory itself, else the code the patch service
//! names for it: [`BlizzardProducts::codes_of`]) is looked up on the patch service
//! through the [`Upstream`] port, one region: `/<code>/versions` for the
//! build players get, `/<code>/bgdl` for a pre-release build the launcher
//! downloads ahead of a launch (when it names another build).
//!
//! A game is current when the stash holds all of its build's install
//! ([`BlizzardPlans`]: the download manifest's files for the configured
//! tags); until that plan is made, its coverage is unknown and the game is
//! not current.
//!
//! Tables are kept for [`BuildOptions::ttl_ms`], failures for
//! [`BuildOptions::retry_ms`], so the page can ask often. Checking does not
//! count as a use of anything in the stash.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_battlenet_domain::tact::{PATCH_HOST, PATCH_PORT, Version, version};
use lanstash_store::store::Store;

use crate::plans::{BlizzardPlans, key};
use crate::products::BlizzardProducts;
use lanstash_battlenet_domain::plan::Coverage;
use lanstash_ports::prefill::Planning;
use lanstash_ports::upstream::Source;

use lanstash_app::origin::{self, OriginError};
use lanstash_ports::clock::Clock;
use lanstash_ports::upstream::Upstream;

/// How builds are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOptions {
    /// The region whose builds count (`us`, `eu`, `kr`, ...).
    pub region: String,
    /// How long a product's table is kept.
    pub ttl_ms: u64,
    /// How long a failed lookup is kept.
    pub retry_ms: u64,
    /// Battle.net directories that are the launcher's own data, not games;
    /// a trailing `*` matches by prefix (`bnt*`: the launcher and its
    /// bootstrapper, `bnt002` and `bnt004`).
    pub skip: Vec<String>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            region: "us".into(),
            ttl_ms: 10 * 60_000,
            retry_ms: 60_000,
            skip: vec!["catalogs".into(), "configs".into(), "bnt*".into()],
        }
    }
}

/// A build and how much of its install the stash holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildState {
    pub version: Version,
    /// `None` until its plan is made (or when it could not be).
    pub coverage: Option<Coverage>,
    /// Why the plan could not be made.
    pub plan_error: Option<String>,
    /// While its plan is being made, or waits its turn: how far that has come.
    pub planning: Option<Planning>,
}

impl BuildState {
    /// Whether the stash holds all of it.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.coverage.is_some_and(|c| c.full())
    }
}

/// One game's builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildCheck {
    /// The game directory (CDN path).
    pub game: String,
    /// The product code whose version was found.
    pub product: Option<String>,
    /// The current build, if the patch service knows the product.
    pub build: Option<BuildState>,
    /// A pre-release build (`bgdl`) other than the current one.
    pub prerelease: Option<BuildState>,
    /// Why the version is unknown, when lookups failed.
    pub error: Option<String>,
    /// The other products players install from the same directory, each with
    /// its own builds (World of Warcraft: Classic, Classic Era, Forever, ...).
    pub flavours: Vec<Flavour>,
}

/// Another product sharing a game's directory, and its builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flavour {
    /// Its product code (a prefill of this code fills the shared directory).
    pub product: String,
    /// Its name in the launcher, from the catalog.
    pub name: Option<String>,
    pub build: Option<BuildState>,
    pub prerelease: Option<BuildState>,
}

impl BuildCheck {
    /// The current build's version.
    #[must_use]
    pub fn version(&self) -> Option<&Version> {
        self.build.as_ref().map(|b| &b.version)
    }

    /// Whether the stash holds all of the current build.
    #[must_use]
    pub fn current(&self) -> bool {
        self.build.as_ref().is_some_and(BuildState::complete)
    }
}

/// A product's table row as last looked up: `Ok(None)` when the patch service has none.
type Looked = Result<Option<Version>, String>;

pub struct BlizzardBuilds {
    upstream: Arc<dyn Upstream>,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    opts: BuildOptions,
    plans: Option<Arc<BlizzardPlans>>,
    products: Option<Arc<BlizzardProducts>>,
    /// By `<code>/<table>`.
    tables: Mutex<BTreeMap<String, (u64, Looked)>>,
}

impl std::fmt::Debug for BlizzardBuilds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlizzardBuilds").field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl BlizzardBuilds {
    #[must_use]
    pub fn new(upstream: Arc<dyn Upstream>, store: Arc<Store>, clock: Arc<dyn Clock>, opts: BuildOptions) -> Self {
        Self { upstream, store, clock, opts, plans: None, products: None, tables: Mutex::new(BTreeMap::new()) }
    }

    /// Measures builds against their plans; without, no game is current.
    #[must_use]
    pub fn with_plans(mut self, plans: Arc<BlizzardPlans>) -> Self {
        self.plans = Some(plans);
        self
    }

    /// Finds the product of a game directory the built-in table does not
    /// know in the catalog (a game not in the stash yet).
    #[must_use]
    pub fn with_products(mut self, products: Arc<BlizzardProducts>) -> Self {
        self.products = Some(products);
        self
    }

    /// The plans builds are measured against.
    #[must_use]
    pub fn plans(&self) -> Option<&Arc<BlizzardPlans>> {
        self.plans.as_ref()
    }

    /// The region builds are checked in.
    #[must_use]
    pub fn region(&self) -> &str {
        &self.opts.region
    }

    /// Every Battle.net game in the stash, but the launcher's own data.
    #[must_use]
    pub fn check_all(&self) -> Vec<BuildCheck> {
        self.store
            .usage()
            .into_keys()
            .filter(|g| g.service == "blizzard" && !self.skipped(&g.game))
            .map(|g| self.check(&g.game))
            .collect()
    }

    /// Whether a directory is the launcher's own data, not a game.
    #[must_use]
    pub fn skipped(&self, game: &str) -> bool {
        self.opts.skip.iter().any(|s| match s.strip_suffix('*') {
            Some(prefix) => game.starts_with(prefix),
            None => s == game,
        })
    }

    /// One game.
    #[must_use]
    pub fn check(&self, game: &str) -> BuildCheck {
        let mut error = None;
        // The main product the patch service names for the directory first, then the directory as a code.
        let mut codes: Vec<String> = self.products.as_ref().and_then(|p| p.code_of(game)).into_iter().collect();
        if !codes.iter().any(|c| c == game) {
            codes.push(game.to_owned());
        }
        for code in &codes {
            let code = code.as_str();
            match self.table(code, "versions") {
                Ok(Some(v)) => {
                    // A pre-release lookup that fails is no pre-release.
                    let pre = self.table(code, "bgdl").ok().flatten().filter(|p| p.build_config != v.build_config);
                    return BuildCheck {
                        game: game.into(),
                        product: Some(code.into()),
                        build: Some(self.state(code, v)),
                        prerelease: pre.map(|p| self.state(code, p)),
                        error: None,
                        flavours: self.flavours(game, code),
                    };
                }
                Ok(None) => {}
                Err(e) => error = Some(e),
            }
        }
        BuildCheck { game: game.into(), product: None, build: None, prerelease: None, error, flavours: Vec::new() }
    }

    /// The products other than `main` under directory `game`, with their builds
    /// (one the patch service has no version of in the region is left out).
    fn flavours(&self, game: &str, main: &str) -> Vec<Flavour> {
        let Some(products) = &self.products else { return Vec::new() };
        let listed = products.list().unwrap_or_default();
        products
            .codes_of(game)
            .into_iter()
            .filter(|c| c != main)
            .filter_map(|code| {
                let v = self.table(&code, "versions").ok().flatten()?;
                let pre = self.table(&code, "bgdl").ok().flatten().filter(|p| p.build_config != v.build_config);
                let name = listed.iter().find(|p| p.code == code).and_then(|p| p.name.clone());
                Some(Flavour {
                    build: Some(self.state(&code, v)),
                    prerelease: pre.map(|p| self.state(&code, p)),
                    name,
                    product: code,
                })
            })
            .collect()
    }

    fn state(&self, product: &str, version: Version) -> BuildState {
        let made = self.plans.as_ref().and_then(|p| p.ready(product, &version).map(|m| (p, m)));
        let (coverage, plan_error) = match made {
            Some((p, Ok(plan))) => (Some(p.coverage(&plan)), None),
            Some((_, Err(e))) => (None, Some(e)),
            None => (None, None),
        };
        let planning = match (&self.plans, coverage, &plan_error) {
            (Some(p), None, None) => p.progress(&key(&version, &[])),
            _ => None,
        };
        BuildState { version, coverage, plan_error, planning }
    }

    fn table(&self, code: &str, table: &str) -> Looked {
        let now = self.clock.now_ms();
        let key = format!("{code}/{table}");
        if let Some((at, looked)) = self.lock().get(&key) {
            let ttl = if looked.is_ok() { self.opts.ttl_ms } else { self.opts.retry_ms };
            if now.saturating_sub(*at) < ttl {
                return looked.clone();
            }
        }
        let src = Source { host: PATCH_HOST.into(), port: Some(PATCH_PORT), target: format!("/{key}") };
        let looked = match origin::get(self.upstream.as_ref(), &src, 1 << 20) {
            Ok(body) => Ok(version(&String::from_utf8_lossy(&body), &self.opts.region)),
            Err(OriginError::Missing) => Ok(None),
            Err(OriginError::Failed(e)) => Err(e),
        };
        self.lock().insert(key, (now, looked.clone()));
        looked
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, (u64, Looked)>> {
        self.tables.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
