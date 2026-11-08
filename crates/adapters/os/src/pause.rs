//! The [`Pause`] port as a real sleep of the calling thread.

use std::time::Duration;

use lanstash_ports::clock::Pause;

/// Sleeps the calling thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sleep;

impl Pause for Sleep {
    fn pause_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}
