//! A daily window of time, written `HH:MM-HH:MM` in UTC (`00:00-04:00`); it
//! may run past midnight (`22:00-03:00`), and `00:00-24:00` is the whole day.

/// Minutes in a day.
const DAY: u32 = 24 * 60;

/// A daily window: from `start` up to `end`, minutes after midnight UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    start: u32,
    end: u32,
}

/// `HH:MM` as minutes after midnight (`24:00` is the end of the day).
fn minutes(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    let at = h.checked_mul(60)?.checked_add(m)?;
    (m < 60 && at <= DAY).then_some(at)
}

impl Window {
    /// `HH:MM-HH:MM`; `None` when it is not one, or empty (start = end).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (a, b) = s.split_once('-')?;
        let (start, end) = (minutes(a)?, minutes(b)?);
        (start != end && start < DAY).then_some(Self { start, end })
    }

    /// Whether `now_ms` (Unix milliseconds) is inside the window.
    #[must_use]
    pub fn contains(&self, now_ms: u64) -> bool {
        let at = u32::try_from((now_ms / 60_000) % u64::from(DAY)).unwrap_or(0);
        if self.start < self.end { (self.start..self.end).contains(&at) } else { at >= self.start || at < self.end }
    }
}
