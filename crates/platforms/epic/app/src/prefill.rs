//! Prefilling an Epic game ([`EpicPlanner`]): the app's current build, asked
//! of Epic as an account that owns it, names its manifest on each CDN (with a
//! signature where the CDN wants one); the manifest, fetched over HTTPS, lists
//! the build's chunks, each a job at its path under the manifest's directory,
//! fetched from the CDNs over plain HTTP (filed by the cache as the launcher's
//! downloads are). Each chunk is checked before it is kept: its data's SHA-1 is
//! the manifest's ([`check_chunk`]).

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_app::prefill::{Job, PieceCheck, Planner, Work};
use lanstash_domain::object::object_key;
use lanstash_epic::library::parse_manifest_urls;
use lanstash_epic::manifest::{Manifest, check_chunk, parse};
use lanstash_ports::prefill::{Planning, StartError};
use lanstash_ports::web::{Tls, Web, WebRequest};

use crate::library::EpicLibrary;

/// A chunk file is held whole to be checked; one larger than this is not a chunk.
const MAX_CHUNK: usize = 64 << 20;

/// The streaming check of one chunk: its bytes held, then checked against the manifest's SHA-1.
struct ChunkCheck {
    sha: [u8; 20],
    bytes: Vec<u8>,
    over: bool,
}

impl PieceCheck for ChunkCheck {
    fn feed(&mut self, data: &[u8]) {
        if self.bytes.len() + data.len() > MAX_CHUNK {
            self.over = true;
        } else {
            self.bytes.extend_from_slice(data);
        }
    }
    fn finish(&self) -> bool {
        !self.over && check_chunk(&self.bytes, &self.sha)
    }
}

/// A game's current build, as its manifest says.
#[derive(Debug, Clone)]
pub struct Current {
    pub version: String,
    pub manifest: Manifest,
    /// The manifest's directory on the CDNs (where the chunks' paths start).
    pub dir: String,
    pub hosts: Vec<String>,
}

impl Current {
    /// The build's chunks as the stash keys them, with their sizes (what a download of it fetches).
    #[must_use]
    pub fn chunk_keys(&self) -> Vec<(String, u64)> {
        let level = self.manifest.feature_level;
        self.manifest
            .chunks
            .iter()
            .map(|k| (object_key("epicgames", &format!("{}{}", self.dir, k.path(level))), k.file_size))
            .collect()
    }
}

/// Epic's side of prefill.
pub struct EpicPlanner {
    library: Arc<EpicLibrary>,
    web: Arc<dyn Web>,
    planning: Mutex<Option<Planning>>,
}

impl std::fmt::Debug for EpicPlanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpicPlanner").finish_non_exhaustive()
    }
}

impl EpicPlanner {
    #[must_use]
    pub fn new(library: Arc<EpicLibrary>, web: Arc<dyn Web>) -> Self {
        Self { library, web, planning: Mutex::new(None) }
    }

    /// `game`'s current build: its manifest (from the first CDN that gives one), its directory
    /// on the CDNs, and the CDNs.
    ///
    /// # Errors
    /// Not in the library, or no manifest to be had.
    pub fn current(&self, game: &str) -> Result<Current, String> {
        let (account, asset) = self.library.app(game).ok_or(format!("{game} is not in the Epic library"))?;
        let urls = parse_manifest_urls(&self.library.build(&account, &asset)?);
        let mut last = String::from("Epic names no manifest for its build");
        for u in &urls {
            match self.fetch(&u.host, &u.target).and_then(|b| parse(&b)) {
                Ok(manifest) => {
                    let mut hosts: Vec<String> = Vec::new();
                    for x in &urls {
                        if !hosts.contains(&x.host) {
                            hosts.push(x.host.clone());
                        }
                    }
                    return Ok(Current { version: asset.build_version, manifest, dir: u.dir.clone(), hosts });
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// A manifest's bytes from `host` and `target`, over HTTPS.
    fn fetch(&self, host: &str, target: &str) -> Result<Vec<u8>, String> {
        let req = WebRequest {
            method: "GET",
            host,
            port: 443,
            target,
            headers: &[],
            body: &[],
            tls: Some(Tls { pin: None, unchecked: false }),
        };
        let r = self.web.send(&req).map_err(|e| format!("{host}: {e}"))?;
        if r.status == 200 { Ok(r.body) } else { Err(format!("{host} answered {} for the manifest", r.status)) }
    }
}

impl Planner for EpicPlanner {
    fn admit(&self, game: &str) -> Result<(), StartError> {
        match self.library.app(game) {
            Some((_, a)) if !a.build_version.is_empty() => Ok(()),
            _ => Err(StartError::NotAGame),
        }
    }

    fn work(&self, game: &str, prerelease: bool, version: &dyn Fn(&str)) -> Result<Work, String> {
        if prerelease {
            return Err("Epic games have no pre-release build here".into());
        }
        let (_, asset) = self.library.app(game).ok_or(format!("{game} is not in the Epic library"))?;
        version(&asset.build_version);
        *self.planning.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(Planning { done: 0, total: 1, step: None });
        let found = self.current(game);
        *self.planning.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let Current { manifest, dir, hosts, .. } = found?;
        let jobs: Vec<Job> = manifest
            .chunks
            .iter()
            .map(|c| Job { target: format!("{dir}{}", c.path(manifest.feature_level)), range: None })
            .collect();
        let bytes = manifest.chunks.iter().map(|c| c.file_size).sum();
        let shas: Arc<Vec<[u8; 20]>> = Arc::new(manifest.chunks.iter().map(|c| c.sha1).collect());
        Ok(Work {
            hosts,
            jobs,
            bytes,
            check: Box::new(move |i, _, _, _| {
                Box::new(ChunkCheck { sha: shas.get(i).copied().unwrap_or([0; 20]), bytes: Vec::new(), over: false })
            }),
            parts: Box::new(|_| Vec::new()),
            // Chunks are checked against the manifest before they are kept.
            check_cdn: true,
        })
    }

    fn planning(&self) -> Option<Planning> {
        *self.planning.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
