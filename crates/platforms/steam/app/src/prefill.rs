//! Prefill for Steam: an app's install (its depots for the configured OS,
//! language and architecture, from the library, with the shared depots other apps
//! provide) as the generic runner's
//! work: one request per unique chunk of each depot's current manifest,
//! asked of the cache as `lancache.steamcontent.com`, as a client behind a
//! lancache asks (the content servers Steam lists give the manifests).
//! From lancache, a chunk counts once its CRC-32 and size check
//! ([`ChunkCheck`]). Steam has no pre-release builds here: only the public
//! branch. A prefill may name another operating system's install.

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_app::prefill::{Job, PieceCheck, Planner, Work};
use lanstash_ports::prefill::{Planning, StartError};
use lanstash_steam_domain::apps::{SYSTEMS, Target};
use lanstash_steam_domain::check::ChunkCheck;
use lanstash_steam_domain::manifest::chunk_path;
use lanstash_steam_domain::profile::LANCACHE_HOST;

use crate::library::SteamLibrary;
use crate::manifests::{self, Servers};
use crate::session::Services;

/// Steam's side of prefill.
pub struct SteamPlanner {
    services: Services,
    library: Arc<SteamLibrary>,
    target: Target,
    planning: Mutex<Option<Planning>>,
}

impl std::fmt::Debug for SteamPlanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamPlanner").field("target", &self.target).finish_non_exhaustive()
    }
}

impl SteamPlanner {
    #[must_use]
    pub fn new(services: Services, library: Arc<SteamLibrary>, target: Target) -> Self {
        Self { services, library, target, planning: Mutex::new(None) }
    }

    fn set_planning(&self, p: Option<Planning>) {
        *self.planning.lock().unwrap_or_else(PoisonError::into_inner) = p;
    }
}

struct Checked(ChunkCheck);

impl PieceCheck for Checked {
    fn feed(&mut self, data: &[u8]) {
        self.0.feed(data);
    }
    fn finish(&self) -> bool {
        self.0.finish()
    }
}

impl Planner for SteamPlanner {
    fn admit(&self, game: &str) -> Result<(), StartError> {
        if !self.services.signed_in() {
            return Err(StartError::NotSetUp);
        }
        let id: u32 = game.parse().map_err(|_| StartError::NotAGame)?;
        // Without a library yet, `work` reads one; with one, the app must be in it.
        if let Some(lib) = self.library.get() {
            let installable = lib.app(id).is_some_and(|a| lib.installable(a, &self.target));
            if !installable {
                return Err(StartError::NotAGame);
            }
        }
        Ok(())
    }

    fn work(&self, game: &str, prerelease: bool, version: &dyn Fn(&str)) -> Result<Work, String> {
        self.work_for(game, prerelease, None, version)
    }

    fn installs(&self) -> bool {
        true
    }

    /// `install`: another operating system (`macos`) in place of the one set up; the
    /// language and architecture stay.
    fn work_for(
        &self,
        game: &str,
        prerelease: bool,
        install: Option<&str>,
        version: &dyn Fn(&str),
    ) -> Result<Work, String> {
        if prerelease {
            return Err("Steam games have no pre-release build here (only the public branch)".into());
        }
        let target = match install {
            None => self.target.clone(),
            Some(os) if SYSTEMS.contains(&os) => Target { os: os.to_string(), ..self.target.clone() },
            Some(os) => return Err(format!("{os} is not an operating system Steam installs for")),
        };
        let id: u32 = game.parse().map_err(|_| format!("{game} is not a Steam app id"))?;
        let lib = match self.library.get() {
            Some(l) => l,
            None => self.library.refresh()?,
        };
        let app = lib.app(id).ok_or(format!("app {id} is not in the account's library"))?;
        let depots = lib.install(app, &target);
        if depots.is_empty() {
            return Err(format!("{} has nothing to install for {}", app.title(), target.os));
        }
        version(&app.build.map_or_else(|| "public".to_string(), |b| b.to_string()));
        let total = depots.len() as u64;
        self.set_planning(Some(Planning { done: 0, total }));
        // Each depot as an account whose licenses grant it (a DLC may be another account's).
        let groups = lib.by_account(&depots, &self.services.accounts());
        let (mut jobs, mut sizes) = (Vec::new(), Vec::new());
        let mut servers: Option<Servers> = None;
        let mut done = 0u64;
        let result = groups.iter().try_for_each(|(account, group)| {
            // Built anew should the session break and the work run again.
            let (j, s) = self.services.with_session_as(account, |conn, on| {
                let servers = servers.get_or_insert_with(|| Servers::new(on.cell_id));
                let (mut j, mut s) = (Vec::new(), Vec::new());
                for (n, (owner, d)) in group.iter().enumerate() {
                    let gid = d.manifest.ok_or(format!("depot {} has no manifest", d.id))?;
                    let (up, lists) = (self.services.upstream.as_ref(), &self.services.lists);
                    for c in manifests::chunks(conn, up, lists, servers, (*owner, d.id, gid), true)? {
                        j.push(Job { target: chunk_path(d.id, &c), range: None });
                        s.push(u64::from(c.compressed));
                    }
                    self.set_planning(Some(Planning { done: done + n as u64 + 1, total }));
                }
                Ok((j, s))
            })?;
            done += group.len() as u64;
            jobs.extend(j);
            sizes.extend(s);
            Ok::<(), String>(())
        });
        self.set_planning(None);
        result?;
        let hosts = vec![LANCACHE_HOST.to_string()];
        let bytes = sizes.iter().sum();
        let sizes = Arc::new(sizes);
        Ok(Work {
            hosts,
            jobs,
            bytes,
            check: Box::new(move |i, _, _, stored| {
                Box::new(Checked(ChunkCheck::new(sizes.get(i).copied().unwrap_or(0), stored)))
            }),
            parts: Box::new(|_| Vec::new()),
            // The cache checks Steam's chunks against `x-content-crc` as it stores them.
            check_cdn: false,
        })
    }

    fn planning(&self) -> Option<Planning> {
        *self.planning.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
