//! Where lanstash looks for newer releases of itself: its repository's releases on GitHub, or
//! nowhere when `check_updates` is off.

use std::sync::Arc;

use lanstash::cli::ConfigArgs;
use lanstash::config::Config;
use lanstash_adapter_admin_api::api::VERSION;
use lanstash_adapter_admin_api::api::version::Newer;
use lanstash_adapter_http::github::GitHubReleases;
use lanstash_adapter_http::web::HttpWeb;
use lanstash_app::releases::{ReleaseWatch, WatchOptions};
use lanstash_ports::clock::Clock;
use lanstash_ports::releases::{NoReleases, Releases};

/// The repository lanstash is released from.
pub const REPO: &str = "darki73/lanstash";

/// The release source the configuration asks for.
#[must_use]
pub fn source(c: &Config) -> Arc<dyn Releases> {
    if c.check_updates {
        let agent = format!("lanstash/{}", env!("CARGO_PKG_VERSION"));
        Arc::new(GitHubReleases::new(Arc::new(HttpWeb::default()), REPO, &agent))
    } else {
        Arc::new(NoReleases)
    }
}

/// The newer release for the API, watched in the background (none at all when `check_updates` is off).
#[must_use]
pub fn newer(c: &Config, clock: &Arc<dyn Clock>) -> Newer {
    if !c.check_updates {
        return Arc::new(|| None);
    }
    let watch = ReleaseWatch::new(source(c), Arc::clone(clock), WatchOptions::default());
    Arc::new(move || watch.newer_than(VERSION))
}

/// The release source for `lanstash doctor`: none when the configuration cannot be read (doctor
/// reports that itself).
#[must_use]
pub fn for_doctor(args: &ConfigArgs) -> Arc<dyn Releases> {
    super::config(args).map_or_else(|_| Arc::new(NoReleases) as Arc<dyn Releases>, |c| source(&c))
}
