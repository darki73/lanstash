//! lanstash's own versions, `major.minor.patch` (a release tag has a `v` in front: `v0.2.0`;
//! anything after a `-` or `+`, `0.2.0-dev`, is not compared).

/// The three numbers of a version, if it is one.
#[must_use]
pub fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim();
    let v = v.strip_prefix('v').unwrap_or(v);
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// Whether `candidate` (a release) is newer than `running`; never when either is no version.
#[must_use]
pub fn newer(candidate: &str, running: &str) -> bool {
    matches!((parse(candidate), parse(running)), (Some(c), Some(r)) if c > r)
}
