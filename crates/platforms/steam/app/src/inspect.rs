//! Why lancache's old cache gives an app's chunks or not: a sample of the
//! current install's chunks, each read the way an import reads it
//! ([`LancacheRead`] and [`ChunkCheck`]), sorted by what happened. A
//! diagnostic: the probe ([`crate::lancache`]) only looks whether a chunk's
//! first slice exists, so it counts chunks an import cannot take.

use std::collections::BTreeMap;
use std::sync::Arc;

use lanstash_app::lancache_read::{LancacheRead, Next};
use lanstash_domain::json::string;
use lanstash_domain::segment::meta::decode_meta;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_steam_domain::apps::Target;
use lanstash_steam_domain::check::ChunkCheck;
use lanstash_steam_domain::manifest::{Chunk, chunk_path};
use lanstash_steam_domain::share::{first_slice, sample};

use crate::library::SteamLibrary;
use crate::manifests;
use crate::session::Services;

/// Chunks read per inspection.
pub const SAMPLE: usize = 64;

/// What reading one chunk out of lancache gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    /// No first slice: lancache does not hold it.
    Absent,
    /// A slice is missing, or a stored answer is not a 200 or 206.
    Unusable,
    /// A slice could not be read, or contradicts its key or the total.
    Error,
    /// Read whole, but no `x-content-crc` was stored with it.
    NoCrc,
    /// Read whole, but not of the manifest's size.
    Size,
    /// Read whole, of the right size, but its CRC is not the stored one.
    Crc,
    /// Read whole and passed its check: an import takes it.
    Ok,
}

impl Outcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Unusable => "unusable",
            Self::Error => "error",
            Self::NoCrc => "no_crc",
            Self::Size => "size",
            Self::Crc => "crc",
            Self::Ok => "ok",
        }
    }
}

/// Reads chunk `c` of `depot` out of lancache as an import does: what happened, and a detail.
#[must_use]
pub fn classify(read: &LancacheRead, source: &dyn NginxCacheSource, depot: u32, c: &Chunk) -> (Outcome, String) {
    let target = chunk_path(depot, c);
    match source.exists(&first_slice(depot, c)) {
        Ok(true) => {}
        Ok(false) => return (Outcome::Absent, target),
        Err(e) => return (Outcome::Error, format!("{target}: {e}")),
    }
    let mut slices = match read.open(&target, None) {
        Ok(Some(s)) => s,
        Ok(None) => return (Outcome::Unusable, target),
        Err(e) => return (Outcome::Error, e),
    };
    let size = u64::from(c.compressed);
    let mut check = ChunkCheck::new(size, &slices.meta);
    let has_crc = decode_meta(&slices.meta).any(|(n, _)| n.eq_ignore_ascii_case("x-content-crc"));
    let mut len = 0u64;
    loop {
        match slices.next_piece() {
            Ok(Next::Bytes(_, b)) => {
                check.feed(&b);
                len += b.len() as u64;
            }
            Ok(Next::Gone) => return (Outcome::Unusable, format!("{target}: a slice went")),
            Ok(Next::End) => break,
            Err(e) => return (Outcome::Error, e),
        }
    }
    if !has_crc {
        (Outcome::NoCrc, target)
    } else if len != size {
        (Outcome::Size, format!("{target}: {len} bytes in lancache, {size} in the manifest"))
    } else if check.finish() {
        (Outcome::Ok, target)
    } else {
        (Outcome::Crc, target)
    }
}

/// Per outcome: chunks, their bytes (the manifest's), and the first one's detail.
pub type Tally = BTreeMap<Outcome, (u64, u64, String)>;

/// The inspection view.
#[must_use]
pub fn view(game: &str, sampled: usize, tally: &Tally) -> String {
    let parts: Vec<String> = tally
        .iter()
        .map(|(o, (n, bytes, example))| {
            format!("{}:{{\"chunks\":{n},\"bytes\":{bytes},\"example\":{}}}", string(o.as_str()), string(example))
        })
        .collect();
    format!("{{\"game\":{},\"sampled\":{sampled},\"outcomes\":{{{}}}}}\n", string(game), parts.join(","))
}

/// Inspects apps of the library.
pub struct LancacheInspect {
    services: Services,
    library: Arc<SteamLibrary>,
    source: Arc<dyn NginxCacheSource>,
    read: LancacheRead,
    target: Target,
}

impl std::fmt::Debug for LancacheInspect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LancacheInspect").finish_non_exhaustive()
    }
}

impl LancacheInspect {
    #[must_use]
    pub fn new(
        services: Services,
        library: Arc<SteamLibrary>,
        source: Arc<dyn NginxCacheSource>,
        read: LancacheRead,
        target: Target,
    ) -> Self {
        Self { services, library, source, read, target }
    }

    /// The inspection of app `game` (an app id), as JSON.
    ///
    /// # Errors
    /// No library yet, not an app of it, or a manifest that could not be read.
    pub fn inspect(&self, game: &str) -> Result<String, String> {
        let lib = self.library.get().ok_or("no Steam library yet")?;
        let id: u32 = game.parse().map_err(|_| format!("{game} is not a Steam app id"))?;
        let app = lib.app(id).ok_or(format!("app {id} is not in the account's library"))?;
        let install = lib.install(app, &self.target);
        let all = manifests::install_chunks(&self.services, &mut None, &lib, &install, true)?;
        let picked = sample(&all, SAMPLE);
        let mut tally = Tally::new();
        for (depot, c) in &picked {
            let (o, detail) = classify(&self.read, self.source.as_ref(), *depot, c);
            let e = tally.entry(o).or_insert((0, 0, detail));
            e.0 += 1;
            e.1 += u64::from(c.compressed);
        }
        Ok(view(game, picked.len(), &tally))
    }
}
