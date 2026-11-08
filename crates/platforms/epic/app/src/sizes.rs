//! The Epic library's download sizes ([`EpicSizes`]): what an install of each
//! game's current build downloads, the sum of its manifest's chunks. Working one
//! out reads the build's manifest from Epic's CDN, so it is done on a thread of
//! its own, one game at a time, and kept between runs by app and build: after a
//! start the sizes are there at once, and only a new build is read again.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_epic::library::Asset;
use lanstash_ports::clock::Pause;
use lanstash_ports::keep::Keep;

use crate::library::EpicLibrary;
use crate::prefill::EpicPlanner;

/// The name the sizes are kept under.
pub const KEPT: &str = "epic-sizes";

/// The rest between two manifests read, and the look when every size is known.
const REST_MS: u64 = 5_000;
const LOOK_MS: u64 = 60_000;

/// The sizes kept, by app: (build, bytes, or why none).
type Sizes = BTreeMap<String, (String, Result<u64, String>)>;

/// The Epic library's download sizes.
pub struct EpicSizes {
    library: Arc<EpicLibrary>,
    planner: Arc<EpicPlanner>,
    keep: Option<Arc<dyn Keep>>,
    sizes: Mutex<Sizes>,
}

impl std::fmt::Debug for EpicSizes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpicSizes").finish_non_exhaustive()
    }
}

impl EpicSizes {
    #[must_use]
    pub fn new(library: Arc<EpicLibrary>, planner: Arc<EpicPlanner>) -> Self {
        Self { library, planner, keep: None, sizes: Mutex::default() }
    }

    /// Keeps the sizes in `keep`, and starts from those kept there.
    #[must_use]
    pub fn with_keep(mut self, keep: Arc<dyn Keep>) -> Self {
        if let Some(b) = keep.load(KEPT).ok().flatten() {
            *self.sizes.get_mut().unwrap_or_else(PoisonError::into_inner) = decode(&String::from_utf8_lossy(&b));
        }
        self.keep = Some(keep);
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Sizes> {
        self.sizes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The download size of `asset`'s current build, once worked out.
    #[must_use]
    pub fn size(&self, asset: &Asset) -> Option<u64> {
        self.lock().get(&asset.app_name).filter(|(v, _)| *v == asset.build_version).and_then(|(_, s)| s.clone().ok())
    }

    /// Works out the size of the first game whose current build has none yet; whether there was one.
    pub fn one(&self) -> bool {
        let todo = self
            .library
            .games()
            .into_iter()
            .find(|a| self.lock().get(&a.app_name).is_none_or(|(v, _)| *v != a.build_version));
        let Some(asset) = todo else { return false };
        let size =
            self.planner.current(&asset.app_name).map(|c| c.manifest.chunks.iter().map(|k| k.file_size).sum::<u64>());
        self.lock().insert(asset.app_name, (asset.build_version, size));
        if let Some(keep) = &self.keep
            && let Err(e) = keep.save(KEPT, encode(&self.lock()).as_bytes())
        {
            println!("Epic sizes not kept: {e}");
        }
        true
    }

    /// Works the sizes out on a thread of its own: one game, a rest, the next.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_going(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        lanstash_app::rounds::forever("epic-sizes", Arc::clone(&pause), move || {
            pause.pause_ms(if me.one() { REST_MS } else { LOOK_MS });
            lanstash_app::rounds::Next::Again
        })
    }
}

/// The kept form: a line per app, `app\tbuild\tbytes` (a size that could not be worked out is
/// not kept: it is tried again after a restart).
#[must_use]
pub fn encode(sizes: &Sizes) -> String {
    sizes.iter().filter_map(|(app, (build, s))| Some(format!("{app}\t{build}\t{}\n", s.as_ref().ok()?))).collect()
}

/// The kept form read back; a line that does not parse is left out.
#[must_use]
pub fn decode(text: &str) -> Sizes {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let (app, build, bytes) = (f.next()?, f.next()?, f.next()?.parse().ok()?);
            Some((app.to_owned(), (build.to_owned(), Ok(bytes))))
        })
        .collect()
}
