//! Riot as the core sees it: an id.

use lanstash_ports::platform::Platform;

/// The Riot platform.
#[derive(Debug, Clone, Copy, Default)]
pub struct RiotPlatform;

impl Platform for RiotPlatform {
    fn id(&self) -> &str {
        "riot"
    }
}
