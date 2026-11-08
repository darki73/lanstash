//! Use case: keeping stashed games current on their own. Every pass looks at
//! each platform's stashed games ([`Platform::build_status`]):
//!   - a game whose current build the stash holds whole is *followed* from
//!     then on (remembered in [`Keep`] across restarts);
//!   - inside the daily window, a followed game whose current build is known
//!     not to be held whole (a new build came out) is queued for a prefill
//!     from the CDN, once per build: a prefill that fails is not retried
//!     until the next build, or a restart.
//!
//! Games never held whole (imported in part from lancache, say) are not
//! followed, so nothing large is fetched that nobody asked for; nor are games
//! deleted whole by hand since: a deleted game seen part-held is not a new
//! build.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::window::Window;
use lanstash_ports::clock::Clock;
use lanstash_ports::keep::Keep;
use lanstash_ports::platform::Platform;
use lanstash_ports::prefill::PrefillFrom;

use crate::delete_game::GameDeleter;

/// The name the followed games are kept under.
pub const KEPT: &str = "auto-update";

#[derive(Default)]
struct State {
    /// `<platform>/<game>`.
    followed: BTreeSet<String>,
    /// `<platform>/<game>@<version>`: builds queued (or refused) already.
    tried: BTreeSet<String>,
    /// `<platform>/<game>`: deleted whole, not followed again while a status from before
    /// the deletion still calls them current; dropped once a status shows them not held
    /// whole, or not at all.
    deleted: BTreeSet<String>,
}

/// What one pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pass {
    /// The games queued, as (platform, game).
    pub queued: Vec<(String, String)>,
    /// The games whose new build could not be queued, as (platform, game, why);
    /// not tried again until their next build, or a restart.
    pub refused: Vec<(String, String, String)>,
}

/// Keeps followed games current.
pub struct AutoUpdate {
    platforms: Vec<Arc<dyn Platform>>,
    clock: Arc<dyn Clock>,
    window: Window,
    keep: Option<Arc<dyn Keep>>,
    deleter: Option<Arc<GameDeleter>>,
    state: Mutex<State>,
}

impl std::fmt::Debug for AutoUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoUpdate").field("window", &self.window).finish_non_exhaustive()
    }
}

impl AutoUpdate {
    #[must_use]
    pub fn new(platforms: Vec<Arc<dyn Platform>>, clock: Arc<dyn Clock>, window: Window) -> Self {
        Self { platforms, clock, window, keep: None, deleter: None, state: Mutex::default() }
    }

    /// Keeps the followed games in `keep` (and starts from those kept there).
    #[must_use]
    pub fn with_keep(mut self, keep: Arc<dyn Keep>) -> Self {
        if let Ok(Some(b)) = keep.load(KEPT) {
            let st = self.state.get_mut().unwrap_or_else(PoisonError::into_inner);
            st.followed = String::from_utf8_lossy(&b).lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
        }
        self.keep = Some(keep);
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The followed games, as `<platform>/<game>`.
    #[must_use]
    pub fn followed(&self) -> Vec<String> {
        self.lock().followed.iter().cloned().collect()
    }

    /// One pass; returns the games queued, as (platform, game).
    pub fn step(&self) -> Vec<(String, String)> {
        self.pass().queued
    }

    /// Stops following the games `deleter` deleted whole (a game deleted by hand is not
    /// fetched back), each pass.
    #[must_use]
    pub fn with_deleter(mut self, deleter: Arc<GameDeleter>) -> Self {
        self.deleter = Some(deleter);
        self
    }

    /// One pass: the games queued, and those whose new build could not be queued, with why.
    pub fn pass(&self) -> Pass {
        let open = self.window.contains(self.clock.now_ms());
        let mut queued = Vec::new();
        let mut refused = Vec::new();
        let mut grew = false;
        if let Some(d) = &self.deleter {
            let gone = d.take_deleted();
            let mut st = self.lock();
            for (platform, game) in gone {
                let key = format!("{platform}/{game}");
                grew |= st.followed.remove(&key);
                st.deleted.insert(key);
            }
        }
        for p in &self.platforms {
            let (Some(status), Some(prefill)) = (p.build_status(), p.prefill()) else { continue };
            {
                // A deleted game the status no longer lists is gone for good.
                let listed: BTreeSet<String> = status.iter().map(|s| format!("{}/{}", p.id(), s.game)).collect();
                let prefix = format!("{}/", p.id());
                self.lock().deleted.retain(|k| !k.starts_with(&prefix) || listed.contains(k));
            }
            for s in status.into_iter().filter(|s| s.known) {
                let key = format!("{}/{}", p.id(), s.game);
                let mut st = self.lock();
                if st.deleted.contains(&key) {
                    if !s.current {
                        st.deleted.remove(&key);
                    }
                    continue;
                }
                if s.current {
                    grew |= st.followed.insert(key);
                    continue;
                }
                if !open || !st.followed.contains(&key) || !st.tried.insert(format!("{key}@{}", s.version)) {
                    continue;
                }
                drop(st);
                match prefill.queue(&s.game, false, PrefillFrom::Cdn) {
                    Ok(_) => queued.push((p.id().to_owned(), s.game)),
                    Err(e) => refused.push((p.id().to_owned(), s.game, format!("{e:?}"))),
                }
            }
        }
        if grew {
            self.save();
        }
        Pass { queued, refused }
    }

    fn save(&self) {
        if let Some(k) = &self.keep {
            let text: String = self.lock().followed.iter().map(|g| format!("{g}\n")).collect();
            // Losing it only means following the games again as they are seen current.
            let _ = k.save(KEPT, text.as_bytes());
        }
    }

    /// Runs a pass every `every_ms` on a thread of its own, printing what it queues
    /// and what it could not.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_going(
        self: &Arc<Self>,
        pause: Arc<dyn lanstash_ports::clock::Pause>,
        every_ms: u64,
    ) -> Result<(), String> {
        let me = Arc::clone(self);
        crate::rounds::forever("auto-update", Arc::clone(&pause), move || {
            let pass = me.pass();
            for (platform, game) in pass.queued {
                println!("auto-update: queued {platform} {game} (a new build)");
            }
            for (platform, game, why) in pass.refused {
                println!("auto-update: could not queue {platform} {game} (a new build): {why}");
            }
            pause.pause_ms(every_ms);
            crate::rounds::Next::Again
        })
    }
}
