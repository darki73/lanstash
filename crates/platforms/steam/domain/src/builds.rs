//! Steam's builds view (`GET /api/v1/builds/steam`), in the shape of
//! Battle.net's so the web UI draws both alike: per app in the stash, its
//! public build, how many bytes of that build's install (the unique chunks
//! of its depots for the configured target) the stash holds, and whether
//! that is all of it.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::json::string;

/// One app's current build and how much of it is stashed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBuild {
    pub app: u32,
    /// Its name, if the library has one.
    pub name: Option<String>,
    /// The public build id.
    pub build: Option<u64>,
    /// Bytes of the install held and wanted; `None` until its manifests are read.
    pub have: Option<u64>,
    pub want: Option<u64>,
    /// Why the install could not be worked out.
    pub error: Option<String>,
    /// What lancache's old cache holds of the build (have, want; an estimate), when probed.
    pub lancache: Option<(u64, u64)>,
}

impl AppBuild {
    /// Whether the stash holds all of it.
    #[must_use]
    pub fn current(&self) -> bool {
        matches!((self.have, self.want), (Some(h), Some(w)) if w > 0 && h >= w)
    }
}

fn opt_num(v: Option<u64>) -> String {
    v.map_or_else(|| String::from("null"), |n| format!("{n}"))
}

fn opt_str(v: Option<&str>) -> String {
    v.map_or_else(|| String::from("null"), string)
}

/// The view: `target` names who the installs are for (`windows`); `lancache`:
/// whether a prefill can take chunks from lancache's old cache first.
#[must_use]
pub fn render(target: &str, lancache: bool, apps: &[AppBuild]) -> String {
    let games: Vec<String> = apps
        .iter()
        .map(|a| {
            let version = a.build.map(|b| format!("{b}"));
            format!(
                "{{\"game\":\"{}\",\"product\":null,\"name\":{},\"version\":{},\"build_config\":null,\"have\":{},\"want\":{},\"plan_error\":{},\"current\":{},\"prerelease\":null,\"error\":null,\"lancache_share\":{}}}",
                a.app,
                opt_str(a.name.as_deref()),
                opt_str(version.as_deref()),
                opt_num(a.have),
                opt_num(a.want),
                opt_str(a.error.as_deref()),
                a.current(),
                a.lancache.map_or_else(
                    || String::from("null"),
                    |(have, want)| format!("{{\"have\":{have},\"want\":{want}}}")
                ),
            )
        })
        .collect();
    format!("{{\"region\":{},\"lancache\":{lancache},\"games\":[{}]}}\n", string(target), games.join(","))
}
