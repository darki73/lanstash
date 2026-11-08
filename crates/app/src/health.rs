//! Health: the checks `lanstash doctor` prints and the web UI's health page
//! shows, as one model ([`Check`]: a level, what, the detail), and the rules
//! both judge by: how many files a process may still open, and what loading
//! the store found (files it could not open because the process may open no
//! more are a limit to raise, not damage).

use std::fmt::Write as _;

use lanstash_domain::evict::KeepFree;
use lanstash_domain::json::string;

/// How a check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

impl Level {
    /// Its name in the API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// One line of the checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub level: Level,
    pub what: String,
    pub detail: String,
}

impl Check {
    pub fn new(level: Level, what: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { level, what: what.into(), detail: detail.into() }
    }
}

/// Files open past this share of the limit: a warning; past [`OPEN_FILES_FAIL`]: a failure.
pub const OPEN_FILES_WARN: f64 = 0.8;
pub const OPEN_FILES_FAIL: f64 = 0.95;

/// How many files `who` has open (`in_use`) of the most it may (`limit`). The store keeps every
/// segment open, so this grows with the stash; at the limit, segments cannot be read.
#[must_use]
pub fn open_files(who: &str, in_use: u64, limit: u64) -> Check {
    let share = if limit == 0 { 1.0 } else { in_use as f64 / limit as f64 };
    let level = if share >= OPEN_FILES_FAIL {
        Level::Fail
    } else if share >= OPEN_FILES_WARN {
        Level::Warn
    } else {
        Level::Ok
    };
    let mut detail = format!("{who}: {in_use} of {limit} ({:.0} %)", share * 100.0);
    if level != Level::Ok {
        detail.push_str("; raise LimitNOFILE in the service's unit");
    }
    Check::new(level, "open files", detail)
}

/// Whether a failure to open a file was the process's open-file limit (EMFILE, OS error 24).
#[must_use]
pub fn is_open_file_limit(error: &str) -> bool {
    error.contains("os error 24") || error.contains("Too many open files")
}

/// What loading the store found: `games` and `bytes` held, files that could not be loaded
/// (`failed`: path, error), and damaged records and journal entries. A file that could not be
/// opened because this process may open no more (`limit` of them) is said apart: it is not damage.
#[must_use]
pub fn store_load(games: u64, bytes: u64, failed: &[(String, String)], damaged: u64, limit: Option<u64>) -> Check {
    let refused = failed.iter().filter(|(_, e)| is_open_file_limit(e)).count();
    let problems = (failed.len() - refused) as u64 + damaged;
    let mut detail = format!("{games} games, {:.2} GB", bytes as f64 / 1e9);
    if problems > 0 {
        let _ = write!(detail, "; {problems} problems (`lanstash store verify` for details)");
    }
    if refused > 0 {
        let may = limit.map_or_else(String::new, |n| format!(" (it may open {n})"));
        let _ = write!(
            detail,
            "; {refused} files not opened: too many open files for this process{may}, \
             so these figures are short; run `ulimit -n 65536` first"
        );
    }
    let level = if problems > 0 || refused > 0 { Level::Warn } else { Level::Ok };
    Check::new(level, "store contents", detail)
}

/// Free space of the store's disk against what `keep_free` keeps free: a warning below it.
#[must_use]
pub fn free_space(free: u64, total: u64, keep_free: Option<KeepFree>) -> Check {
    let need = keep_free.map_or(0, |k| k.bytes(total));
    let keep = match keep_free {
        Some(KeepFree::Percent(p)) => format!("{p}%"),
        Some(KeepFree::Bytes(b)) => format!("{:.0} GB", b as f64 / 1e9),
        None => "off".to_owned(),
    };
    let detail = format!("{:.0} GB free of {:.0} GB (keep_free {keep})", free as f64 / 1e9, total as f64 / 1e9);
    Check::new(if free >= need { Level::Ok } else { Level::Warn }, "free space", detail)
}

/// What the background check (`verify` while serving) found in its last round: `problems`,
/// and whether a round ran yet (`finished_ms` 0: none) or runs now.
#[must_use]
pub fn background_check(problems: u64, finished_ms: u64, running: bool) -> Check {
    let detail = match (problems, finished_ms, running) {
        (0, 0, true) => "its first round is running".to_owned(),
        (0, 0, false) => "no round yet".to_owned(),
        (0, _, _) => "no problems in its last round".to_owned(),
        (n, _, _) => format!("{n} problems (`lanstash store verify` for details)"),
    };
    Check::new(if problems > 0 { Level::Warn } else { Level::Ok }, "background check", detail)
}

/// The checks as JSON: `{"checks":[{"level","what","detail"}]}`.
#[must_use]
pub fn json(checks: &[Check]) -> String {
    let rows: Vec<String> = checks
        .iter()
        .map(|c| {
            format!(
                "{{\"level\":{},\"what\":{},\"detail\":{}}}",
                string(c.level.as_str()),
                string(&c.what),
                string(&c.detail)
            )
        })
        .collect();
    format!("{{\"checks\":[{}]}}\n", rows.join(","))
}
