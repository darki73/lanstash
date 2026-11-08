//! Making a plan: the build's documents read (through the cache for a game the
//! stash holds, else from the CDN alone), then
//! [`lanstash_battlenet_domain::plan::assemble`].

use lanstash_app::origin::{self, OriginError};
use lanstash_app::via_cache::{self, Get, Watch};
use lanstash_battlenet_domain::blte;
use lanstash_battlenet_domain::download::{self, install_tags, retarget, select};
use lanstash_battlenet_domain::locate::{Place, locate};
use lanstash_battlenet_domain::plan::{Build, Plan, assemble};
use lanstash_battlenet_domain::tact::{
    IndexEntry, PATCH_HOST, PATCH_PORT, Version, build_files, cdn_hosts, config_list, config_value, parse_index, target,
};
use lanstash_ports::upstream::Source;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use super::{Inner, lock};

impl Inner {
    /// One file (or range): through the cache, so it is stashed, when the stash holds the game's
    /// directory already (`/tpr/<directory>/...`); else from the CDN only, keeping nothing: looking
    /// at a game's build is not a download of it (a prefill fetches the plan's files, these among them).
    fn fetch(&self, host: &str, target: &str, range: Option<(u64, u64)>) -> Result<Option<Vec<u8>>, String> {
        let dir = target.split('/').nth(2).unwrap_or_default();
        if self.store.holds("blizzard", dir) {
            let get = Get { host, target, range, keep: Some(self.opts.max_file) };
            return via_cache::get(self.cache.as_ref(), get, Watch::default());
        }
        let src = Source { host: host.into(), port: None, target: target.into() };
        match origin::get_range(self.upstream.as_ref(), &src, range, self.opts.max_file) {
            Ok(body) => Ok(Some(body)),
            Err(OriginError::Missing) => Ok(None),
            Err(OriginError::Failed(e)) => Err(format!("{target}: {e}")),
        }
    }

    /// Reads whole files `parallel` at a time, file number `i` from host
    /// `i mod hosts` (so they spread over the hosts whichever worker takes
    /// them); each file's body (`None`: not on the CDN) in the order asked,
    /// `done` counting them. The first failure ends it.
    fn fetch_all(
        &self,
        hosts: &[String],
        targets: &[String],
        done: &AtomicU64,
    ) -> Result<Vec<Option<Vec<u8>>>, String> {
        if hosts.is_empty() {
            return Err("no CDN host".into());
        }
        let out: Vec<Mutex<Option<Vec<u8>>>> = targets.iter().map(|_| Mutex::new(None)).collect();
        let next = AtomicUsize::new(0);
        let failed: Mutex<Option<String>> = Mutex::new(None);
        let workers = self.opts.parallel.clamp(1, 64).min(targets.len().max(1));
        std::thread::scope(|s| {
            for _ in 0..workers {
                let (out, next, failed) = (&out, &next, &failed);
                s.spawn(move || {
                    while lock(failed).is_none() {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(t) = targets.get(i) else { return };
                        let host = &hosts[i % hosts.len()];
                        match self.fetch(host, t, None) {
                            Ok(body) => {
                                *lock(&out[i]) = body;
                                done.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => {
                                lock(failed).get_or_insert(format!("{t}: {e}"));
                            }
                        }
                    }
                });
            }
        });
        if let Some(e) = failed.into_inner().unwrap_or_else(PoisonError::into_inner) {
            return Err(e);
        }
        Ok(out.into_iter().map(|m| m.into_inner().unwrap_or_else(PoisonError::into_inner)).collect())
    }

    /// Makes the plan; `done` of `total` counts the configs and indexes read.
    pub(super) fn make(
        &self,
        product: &str,
        version: &Version,
        install: &[&str],
        (done, total): (&AtomicU64, &AtomicU64),
    ) -> Result<Plan, String> {
        let src = Source { host: PATCH_HOST.into(), port: Some(PATCH_PORT), target: format!("/{product}/cdns") };
        let cdns = origin::get(self.upstream.as_ref(), &src, 1 << 20).map_err(|e| format!("{product}/cdns: {e:?}"))?;
        let (hosts, path) =
            cdn_hosts(&String::from_utf8_lossy(&cdns), &self.opts.region).ok_or(format!("{product}/cdns: no CDN"))?;
        let host = hosts[0].clone();
        let at = |kind: &str, hash: &str, suffix: &str| -> Result<String, String> {
            Ok(format!("{}{suffix}", target(&path, kind, hash).ok_or(format!("bad hash {hash}"))?))
        };
        // Whole files read, in order: (target, size).
        let mut files: Vec<(String, u64)> = Vec::new();
        let mut keep = |targets: &[String], bodies: &[Option<Vec<u8>>]| {
            for (t, b) in targets.iter().zip(bodies) {
                if let Some(b) = b {
                    files.push((t.clone(), b.len() as u64));
                }
            }
        };
        let text = |b: &Option<Vec<u8>>, what: &str| -> Result<String, String> {
            Ok(String::from_utf8_lossy(b.as_deref().ok_or(format!("{what}: not on the CDN"))?).into_owned())
        };
        let configs = [at("config", &version.build_config, "")?, at("config", &version.cdn_config, "")?];
        total.store(configs.len() as u64, Ordering::Relaxed);
        let bodies = self.fetch_all(&hosts, &configs, done)?;
        keep(&configs, &bodies);
        let build_config = text(&bodies[0], "build config")?;
        let cdn_config = text(&bodies[1], "CDN config")?;
        // Every archive's index, the file index, then (a client reads them even for a full
        // install) the patch archives' indexes and the patch file index, kept whole.
        let archive_hashes = config_list(&cdn_config, "archives");
        let file_index = config_value(&cdn_config, "file-index").map(str::trim);
        let patch_file_index = config_value(&cdn_config, "patch-file-index").map(str::trim);
        let patch: Vec<&str> = config_list(&cdn_config, "patch-archives").into_iter().chain(patch_file_index).collect();
        let mut indexed: Vec<String> =
            archive_hashes.iter().map(|a| at("data", a, ".index")).collect::<Result<_, _>>()?;
        if let Some(h) = file_index {
            indexed.push(at("data", h, ".index")?);
        }
        for h in &patch {
            indexed.push(at("patch", h, ".index")?);
        }
        total.fetch_add(indexed.len() as u64, Ordering::Relaxed);
        let bodies = self.fetch_all(&hosts, &indexed, done)?;
        keep(&indexed, &bodies);
        let mut indexes: Vec<Vec<IndexEntry>> = Vec::with_capacity(archive_hashes.len());
        for (a, body) in archive_hashes.iter().zip(&bodies) {
            let body = body.as_deref().ok_or(format!("archive index {a}: not on the CDN"))?;
            indexes.push(parse_index(body).map_err(|e| format!("archive index {a}: {e:?}"))?);
        }
        let file_index = match (file_index, bodies.get(archive_hashes.len())) {
            (Some(_), Some(Some(body))) => Some(parse_index(body).map_err(|e| format!("file index: {e:?}"))?),
            _ => None,
        };
        let archives: Vec<String> = archive_hashes.iter().filter_map(|a| target(&path, "data", a)).collect::<Vec<_>>();
        if archives.len() != archive_hashes.len() {
            return Err("CDN config: bad archive hash".into());
        }
        // The build config's own files, then the download manifest's selection.
        let named = build_files(&build_config);
        let mut wanted: Vec<(String, u64)> = named.iter().map(|(_, k, s)| (k.clone(), *s)).collect();
        let download = named.iter().find(|(n, _, _)| n == "download").ok_or("build config: no download manifest")?;
        let manifest_at = locate(&[(download.1.clone(), download.2)], &indexes, file_index.as_deref());
        let raw = match manifest_at[0].place {
            Place::Archive { archive, offset, size } => {
                self.fetch(&host, &archives[archive], Some((offset, offset + size)))?
            }
            _ => self.fetch(&host, &target(&path, "data", &download.1).ok_or("bad download key")?, None)?,
        }
        .ok_or("download manifest: not on the CDN")?;
        let decoded = blte::decode(&raw, 8 * self.opts.max_file).map_err(|e| format!("download manifest: {e:?}"))?;
        let manifest = download::parse(&decoded).map_err(|e| format!("download manifest: {e:?}"))?;
        // The install set up, or another platform's in its place.
        let chosen = retarget(&manifest, &self.opts.tags, install)?;
        let tags: Vec<&str> = chosen.iter().map(String::as_str).collect();
        wanted.extend(select(&manifest, &tags).into_iter().map(|i| {
            let e = &manifest.entries[i];
            (e.key.clone(), e.size)
        }));
        let located = locate(&wanted, &indexes, file_index.as_deref());
        let build = Build { product: product.to_owned(), version: version.clone(), path, hosts, archives, files };
        let mut plan = assemble(build, &located, self.opts.gap)?;
        plan.installs = install_tags(&manifest);
        Ok(plan)
    }
}
