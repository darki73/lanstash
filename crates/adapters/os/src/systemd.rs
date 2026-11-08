//! The service manager: systemd, through `systemctl`. Used by
//! `lanstash install`, `uninstall` and `doctor`.

use std::time::Duration;

use crate::command::run;

/// Answers `systemctl is-enabled` / `is-active` / `show -p MainPID` for a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitState {
    pub installed: bool,
    pub enabled: bool,
    pub active: bool,
    /// Its main process, while it runs one.
    pub main_pid: Option<u32>,
}

/// `systemctl` with a time limit per call.
#[derive(Debug, Clone, Copy)]
pub struct Systemd {
    pub timeout: Duration,
}

impl Default for Systemd {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(90) }
    }
}

impl Systemd {
    fn ctl(&self, args: &[&str]) -> Result<(), String> {
        let out = run("systemctl", args, self.timeout)?;
        if out.success { Ok(()) } else { Err(format!("systemctl {}: exit code {:?}", args.join(" "), out.code)) }
    }

    /// Whether systemd is there at all (and answers).
    #[must_use]
    pub fn available(&self) -> bool {
        run("systemctl", &["--version"], self.timeout).is_ok_and(|o| o.success)
    }

    /// # Errors
    /// `systemctl` failed.
    pub fn daemon_reload(&self) -> Result<(), String> {
        self.ctl(&["daemon-reload"])
    }

    /// Enables the unit at boot and starts it now.
    ///
    /// # Errors
    /// `systemctl` failed.
    pub fn enable_now(&self, unit: &str) -> Result<(), String> {
        self.ctl(&["enable", "--now", unit])
    }

    /// # Errors
    /// `systemctl` failed.
    pub fn restart(&self, unit: &str) -> Result<(), String> {
        self.ctl(&["restart", unit])
    }

    /// # Errors
    /// `systemctl` failed.
    pub fn stop(&self, unit: &str) -> Result<(), String> {
        self.ctl(&["stop", unit])
    }

    /// Stops it and keeps it from starting at boot.
    ///
    /// # Errors
    /// `systemctl` failed.
    pub fn disable_now(&self, unit: &str) -> Result<(), String> {
        self.ctl(&["disable", "--now", unit])
    }

    /// The unit's state (`installed` = systemd knows a unit file for it).
    #[must_use]
    pub fn state(&self, unit: &str) -> UnitState {
        let word = |args: &[&str]| {
            run("systemctl", args, self.timeout).map(|o| o.stdout.trim().to_owned()).unwrap_or_default()
        };
        let enabled = word(&["is-enabled", unit]);
        let active = word(&["is-active", unit]);
        // `MainPID=1234`; 0 when it runs no process.
        let pid = word(&["show", "-p", "MainPID", unit]);
        UnitState {
            // `is-enabled` prints `disabled`/`enabled`/`static`… for a known unit, nothing or `not-found` otherwise.
            installed: !enabled.is_empty() && enabled != "not-found",
            enabled: enabled == "enabled",
            active: active == "active" || active == "activating" || active == "reloading",
            main_pid: pid.strip_prefix("MainPID=").and_then(|p| p.parse().ok()).filter(|p| *p > 0),
        }
    }
}
