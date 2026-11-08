//! `lanstash install` / `uninstall` / `doctor`, the pure parts: where things
//! go, the systemd unit, the installed configuration, the checklist. The
//! steps that touch the system are in the binary (`setup.rs`).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use lanstash_app::health::{Check, Level};

/// Where an installation lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// systemd unit name (without `.service`).
    pub unit: String,
    pub bin: PathBuf,
    pub config: PathBuf,
    pub unit_file: PathBuf,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            unit: "lanstash".into(),
            bin: "/usr/local/bin/lanstash".into(),
            config: "/etc/lanstash/lanstash.conf".into(),
            unit_file: "/etc/systemd/system/lanstash.service".into(),
        }
    }
}

/// Seconds `lanstash stop` may take before systemd kills the process (which
/// is safe: crash recovery).
pub const STOP_WAIT_SECS: u64 = 60;

/// Admin address added when the configuration has none (needed to stop cleanly).
pub const DEFAULT_ADMIN: &str = "127.0.0.1:8081";

/// A path systemd can take as one unquoted word.
fn unit_path(p: &Path) -> Result<String, String> {
    let s = p.to_str().ok_or_else(|| format!("{}: not UTF-8", p.display()))?;
    // systemd paths are Unix paths, whatever this program was built on.
    if !s.starts_with('/') || s.chars().any(|c| c.is_whitespace() || matches!(c, '%' | '"' | '\'' | '\\' | '$')) {
        return Err(format!("{s}: must be absolute, without spaces or % \" ' \\ $"));
    }
    Ok(s.to_owned())
}

/// The systemd unit. `mounts`: store roots, which must be mounted first.
///
/// # Errors
/// A path systemd could not take as written.
pub fn render_unit(l: &Layout, mounts: &[PathBuf]) -> Result<String, String> {
    let (bin, conf) = (unit_path(&l.bin)?, unit_path(&l.config)?);
    let mut u = String::from(
        "# Written by `lanstash install`; `lanstash uninstall` removes it.\n\
         [Unit]\n\
         Description=lanstash: caching proxy for game downloads\n\
         After=network-online.target remote-fs.target\n\
         Wants=network-online.target\n",
    );
    for m in mounts {
        let _ = writeln!(u, "RequiresMountsFor={}", unit_path(m)?);
    }
    let _ = write!(
        u,
        "\n[Service]\n\
         Type=simple\n\
         ExecStart={bin} serve --config {conf}\n\
         ExecStop={bin} stop --config {conf} --wait {STOP_WAIT_SECS}\n\
         TimeoutStopSec={}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         LimitNOFILE=65536\n\
         \n[Install]\n\
         WantedBy=multi-user.target\n",
        STOP_WAIT_SECS + 30
    );
    Ok(u)
}

/// The configuration to install: the given file's text (if any), then the
/// command-line settings as lines (later lines win), then `admin_listen` if
/// nothing set it.
#[must_use]
pub fn installed_config(file: Option<&str>, flags: &[(&str, &str)], has_admin: bool) -> String {
    let mut t = file.unwrap_or("# lanstash configuration\n").to_owned();
    if !t.ends_with('\n') {
        t.push('\n');
    }
    if !flags.is_empty() {
        t.push_str("\n# from `lanstash install` flags\n");
        for (k, v) in flags {
            let _ = writeln!(t, "{k} = {v}");
        }
    }
    if !has_admin {
        let _ = write!(
            t,
            "\n# added by `lanstash install`: `lanstash stop` goes through it\nadmin_listen = {DEFAULT_ADMIN}\n"
        );
    }
    t
}

/// The checklist ([`lanstash_app::health`]'s checks) and a last line with the counts.
#[must_use]
pub fn render_checks(checks: &[Check]) -> String {
    let mut s = String::new();
    for c in checks {
        let mark = match c.level {
            Level::Ok => "ok  ",
            Level::Warn => "WARN",
            Level::Fail => "FAIL",
        };
        let _ = writeln!(s, "[{mark}] {:<22} {}", c.what, c.detail);
    }
    let n = |l| checks.iter().filter(|c| c.level == l).count();
    let _ = writeln!(s, "{} ok, {} warnings, {} failed", n(Level::Ok), n(Level::Warn), n(Level::Fail));
    s
}
