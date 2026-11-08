//! Battle.net's side of prefill ([`lanstash_app::prefill`]): a game's
//! current build (or its pre-release build) is its plan ([`crate::plans`]:
//! what an install downloads), fetched as its files whole and its archive
//! ranges; for another platform's install (`OSX,arm64`), that install's plan. Every piece, from
//! the CDN or from lancache's cache, is checked
//! against the hashes in its path as it is read, and stored only once it
//! passes ([`lanstash_battlenet_domain::check::Check`]: a whole file, or every
//! file inside an archive range).

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_app::prefill::{Check, Job, Parts, PieceCheck, Planner, Work};
use lanstash_battlenet_domain::check;
use lanstash_battlenet_domain::locate::Range;
use lanstash_battlenet_domain::plan::Plan;
use lanstash_ports::prefill::{Planning, StartError};

use crate::builds::BlizzardBuilds;
use crate::plans::key;

/// Prefill jobs of Battle.net games, from their plans.
#[derive(Debug)]
pub struct BlizzardJobs {
    builds: Arc<BlizzardBuilds>,
    /// The build config of the job's build, once known (for its planning progress).
    building: Mutex<Option<String>>,
}

impl BlizzardJobs {
    #[must_use]
    pub fn new(builds: Arc<BlizzardBuilds>) -> Self {
        Self { builds, building: Mutex::new(None) }
    }

    fn building(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.building.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A piece's check as the runner feeds it.
struct Checked(check::Check);

impl PieceCheck for Checked {
    fn feed(&mut self, data: &[u8]) {
        self.0.feed(data);
    }
    fn finish(&self) -> bool {
        self.0.finish()
    }
    fn checks(&self) -> bool {
        self.0.checks()
    }
}

/// The files inside archive request `r`: their bytes and encoding keys.
fn inside<'a>(plan: &'a Plan, r: &Range) -> Vec<(u64, u64, &'a str)> {
    let first = plan.parts.partition_point(|p| (p.0, p.1.start) < (r.archive, r.start));
    plan.parts[first..]
        .iter()
        .take_while(|p| p.0 == r.archive && p.1.end <= r.end)
        .map(|p| (p.1.start, p.1.end, p.2.as_str()))
        .collect()
}

impl Planner for BlizzardJobs {
    fn admit(&self, game: &str) -> Result<(), StartError> {
        if self.builds.skipped(game) {
            return Err(StartError::NotAGame);
        }
        if self.builds.plans().is_none() {
            return Err(StartError::NotSetUp);
        }
        Ok(())
    }

    fn work(&self, game: &str, prerelease: bool, version: &dyn Fn(&str)) -> Result<Work, String> {
        self.work_for(game, prerelease, None, version)
    }

    fn installs(&self) -> bool {
        true
    }

    /// `install`: another platform's tags (`OSX,arm64`) in place of those set up.
    fn work_for(
        &self,
        game: &str,
        prerelease: bool,
        install: Option<&str>,
        version: &dyn Fn(&str),
    ) -> Result<Work, String> {
        let install: Vec<&str> = install.map_or_else(Vec::new, |i| i.split(',').filter(|t| !t.is_empty()).collect());
        *self.building() = None;
        let check = self.builds.check(game);
        let build = if prerelease { check.prerelease } else { check.build };
        let (Some(product), Some(build)) = (check.product, build) else {
            return Err(check.error.unwrap_or_else(|| {
                if prerelease {
                    format!("{game}: no pre-release build")
                } else {
                    format!("{game}: not a product the patch service knows")
                }
            }));
        };
        version(&build.version.name);
        *self.building() = Some(key(&build.version, &install));
        let plans = self.builds.plans().ok_or("no plans")?;
        // The plan's files start with the documents it was made from: the prefill stashes them too.
        let plan = plans.plan_for(&product, &build.version, &install)?;
        let whole = plan.files.iter().map(|(t, _)| Job { target: t.clone(), range: None });
        let ranges = plan
            .requests
            .iter()
            .map(|r| Job { target: plan.archives[r.archive].clone(), range: Some((r.start, r.end)) });
        let jobs: Vec<Job> = whole.chain(ranges).collect();
        let hosts = if plan.hosts.is_empty() { vec![plan.host.clone()] } else { plan.hosts.clone() };
        let bytes = plan.bytes();
        let files = plan.files.len();
        let parted = Arc::clone(&plan);
        // A whole file, or the files of an archive range within `start..end`.
        let check: Check = Box::new(move |i: usize, start: u64, end: u64, _stored: &[u8]| {
            let c = match i.checked_sub(files) {
                None => check::Check::whole(&plan.files[i].0),
                Some(r) => plan.requests.get(r).map_or(check::Check::Never, |r| {
                    let within: Vec<_> =
                        inside(&plan, r).into_iter().filter(|(a, b, _)| start <= *a && *b <= end).collect();
                    check::Check::range(start, &within)
                }),
            };
            Box::new(Checked(c))
        });
        // The files inside an archive range, each a part lancache may hold whole.
        let parts: Parts = Box::new(move |i: usize| {
            i.checked_sub(files)
                .and_then(|r| parted.requests.get(r))
                .map(|r| inside(&parted, r).into_iter().map(|(a, b, _)| (a, b)).collect())
                .unwrap_or_default()
        });
        // Every file's name is a hash of its content: what the CDN sends is checked too.
        Ok(Work { hosts, jobs, bytes, check, parts, check_cdn: true })
    }

    fn planning(&self) -> Option<Planning> {
        let key = self.building().clone()?;
        self.builds.plans()?.progress(&key)
    }
}
