//! Cover art for the web UI: fetched once from the
//! platform's CDN through the [`Upstream`] port and kept in memory, so the
//! browser never talks to the CDN and repeat views cost nothing.
//!
//! Where an image is depends on the platform: each registers an
//! [`ArtSource`] that names the address of an image of a kind ([`ArtKind`]:
//! a header or an icon), or fetches the image itself where its CDN answers
//! only over HTTPS (Epic). A platform without one has no art.
//!
//! Bounded: an image over [`ArtOptions::max_image`] is refused, and the cache
//! drops the least recently used images past [`ArtOptions::max_total`]. A
//! miss (the CDN has no image) is remembered for [`ArtOptions::miss_ttl_ms`],
//! an upstream failure for [`ArtOptions::fail_ttl_ms`]. The image type comes
//! from the bytes (Battle.net sends `binary/octet-stream`); anything but PNG,
//! JPEG, WebP or GIF is refused.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::art;

use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::clock::Clock;
use lanstash_ports::upstream::{Source, Upstream};

use crate::origin::{self, OriginError};

/// Limits of the art cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtOptions {
    /// Largest image accepted, in bytes.
    pub max_image: usize,
    /// Bytes of images kept in memory.
    pub max_total: usize,
    /// How long "the CDN has no image" is remembered.
    pub miss_ttl_ms: u64,
    /// How long an upstream failure is remembered before trying again.
    pub fail_ttl_ms: u64,
}

impl Default for ArtOptions {
    fn default() -> Self {
        Self { max_image: 8 << 20, max_total: 128 << 20, miss_ttl_ms: 6 * 3_600_000, fail_ttl_ms: 60_000 }
    }
}

/// One image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

type Found = Result<Arc<Image>, ArtError>;
type Key = (String, String, ArtKind);

struct Entry {
    found: Found,
    at_ms: u64,
    used: u64,
}

#[derive(Default)]
struct Cache {
    entries: BTreeMap<Key, Entry>,
    bytes: usize,
    tick: u64,
}

/// The art use case.
pub struct Art {
    upstream: Arc<dyn Upstream>,
    clock: Arc<dyn Clock>,
    opts: ArtOptions,
    sources: BTreeMap<String, Arc<dyn ArtSource>>,
    cache: Mutex<Cache>,
}

impl std::fmt::Debug for Art {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Art").field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl Art {
    #[must_use]
    pub fn new(upstream: Arc<dyn Upstream>, clock: Arc<dyn Clock>, opts: ArtOptions) -> Self {
        Self { upstream, clock, opts, sources: BTreeMap::new(), cache: Mutex::new(Cache::default()) }
    }

    /// Serves the art of `platform` from `source`.
    #[must_use]
    pub fn with_source(mut self, platform: &str, source: Arc<dyn ArtSource>) -> Self {
        self.sources.insert(platform.to_owned(), source);
        self
    }

    /// The `kind` image of game `id` of `platform`.
    ///
    /// # Errors
    /// [`ArtError`]: no source, no image, or the CDN failed.
    pub fn get(&self, platform: &str, id: &str, kind: ArtKind) -> Found {
        let key = (platform.to_owned(), id.to_owned(), kind);
        let now = self.clock.now_ms();
        if let Some(found) = self.cached(&key, now) {
            return found;
        }
        let source = self.sources.get(platform);
        let own = source.and_then(|s| s.fetch(id, kind));
        let found = match own {
            Some(body) => body.and_then(|b| typed(b, id)),
            None => match self.locate(platform, id, kind).and_then(|src| self.fetch(&src)) {
                // Missing there: where else the source says to look, if anywhere.
                Err(ArtError::Missing) => match source.and_then(|s| s.fallback(id, kind)) {
                    Some(other) => self.fetch(&other),
                    None => Err(ArtError::Missing),
                },
                other => other,
            },
        }
        .map(Arc::new);
        if found != Err(ArtError::Unknown) {
            self.keep(key, found.clone(), now);
        }
        found
    }

    /// Bytes of images held now.
    #[must_use]
    pub fn held(&self) -> usize {
        self.lock().bytes
    }

    fn locate(&self, platform: &str, id: &str, kind: ArtKind) -> Result<Source, ArtError> {
        self.sources.get(platform).ok_or(ArtError::Unknown)?.locate(id, kind)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn cached(&self, key: &Key, now: u64) -> Option<Found> {
        let mut c = self.lock();
        c.tick += 1;
        let tick = c.tick;
        let e = c.entries.get_mut(key)?;
        let ttl = match &e.found {
            Ok(_) => u64::MAX,
            Err(ArtError::Missing) => self.opts.miss_ttl_ms,
            Err(_) => self.opts.fail_ttl_ms,
        };
        if now.saturating_sub(e.at_ms) >= ttl {
            return None;
        }
        e.used = tick;
        Some(e.found.clone())
    }

    fn keep(&self, key: Key, found: Found, now: u64) {
        let size = found.as_ref().map_or(0, |i| i.body.len());
        let mut c = self.lock();
        c.tick += 1;
        let used = c.tick;
        if let Some(old) = c.entries.insert(key, Entry { found, at_ms: now, used }) {
            c.bytes -= old.found.as_ref().map_or(0, |i| i.body.len());
        }
        c.bytes += size;
        while c.bytes > self.opts.max_total {
            let Some(oldest) =
                c.entries.iter().filter(|(_, e)| e.found.is_ok()).min_by_key(|(_, e)| e.used).map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(e) = c.entries.remove(&oldest) {
                c.bytes -= e.found.as_ref().map_or(0, |i| i.body.len());
            }
        }
    }

    fn fetch(&self, src: &Source) -> Result<Image, ArtError> {
        let body = origin::get(self.upstream.as_ref(), src, self.opts.max_image).map_err(|e| match e {
            OriginError::Missing => ArtError::Missing,
            OriginError::Failed(m) => ArtError::Upstream(m),
        })?;
        typed(body, &format!("{}{}", src.host, src.target))
    }
}

/// `body` as an image of its type; `what` names it when it is none.
fn typed(body: Vec<u8>, what: &str) -> Result<Image, ArtError> {
    let content_type = art::image_type(&body).ok_or_else(|| ArtError::Upstream(format!("{what}: not an image")))?;
    Ok(Image { content_type, body })
}
