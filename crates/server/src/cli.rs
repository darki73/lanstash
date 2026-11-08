//! Command-line parsing (pure: arguments in, [`Command`] out).

use std::path::PathBuf;

use crate::config::{Settings, is_switch};

pub const USAGE: &str = "\
lanstash: a caching proxy for game downloads

USAGE:
    lanstash serve [--config FILE] [--KEY VALUE]...
        serve and cache; every configuration key can be given as a flag (later wins):
          --store DIR[,DIR…]      store root (extra entries: other mounts of the same directory for reads)
          --store.<service> DIR   root for one service
          --upstream-dns IP[,IP]  DNS for origin names (not the local DNS, which points at the cache)
          --cache-domains DIR     directory with <service>.txt host lists
          --listen ADDR           default 0.0.0.0:80
          --access-log FILE, --access-log-headers, --event-log FILE, --stats-secs N, …
    lanstash import --nginx-cache DIR --dirs LIST [--threads N] [--slice SIZE] [--config FILE] [--KEY VALUE]...
        import an nginx/lancache cache (read-only); --dirs: level directories, e.g. 00,1a or 00-ff
    lanstash store stats [--config FILE] [--KEY VALUE]...
        objects and bytes per service and game
    lanstash store verify [--threads N] [--rate MB/S] [--accept] [--config FILE] [--KEY VALUE]...
        read every stored byte and check it against its checksum (read-only; safe while serving),
        as fast as the disks go or at most --rate MB a second: it says how much it reads and asks
        first (--accept: does not ask, for systemd and scripts); `verify_every` checks the same
        slowly in the background
    lanstash stop [--wait SECS] [--config FILE] [--KEY VALUE]...
        stop the running lanstash cleanly (through admin_listen, from this machine)
    lanstash prefill <platform>/<game> [--pre] [--from lancache|cdn] [--detach] [--config FILE] [--KEY VALUE]...
        prefill a game's current (--pre: pre-release) build in the running lanstash and follow it;
        --from lancache: take what lancache's old cache (nginx_cache) holds, checked, the rest from the CDN
    lanstash prefill stop [--config FILE] [--KEY VALUE]...
        stop the running prefill
    lanstash steam login [--password] | status | logout [--config FILE] [--KEY VALUE]...
        login: sign in to Steam once by scanning a QR code with the Steam mobile app (--password:
        account, password and Steam Guard instead) and keep the refresh token in `secrets` (no
        password is kept); the web UI signs in the same way; status: log on with the kept token;
        logout: forget it (default config: /etc/lanstash/lanstash.conf)
    sudo lanstash user add <name> [--admin] | passwd <name> | role <name> admin|viewer | remove <name> | list
        lanstash's own accounts for the web UI and the API, kept in `secrets` (passwords asked on the
        terminal, kept only as hashes); add makes a viewer unless --admin; the last admin stays
        (default config: /etc/lanstash/lanstash.conf)
    lanstash seg dump <file> [--verify] [--limit N]   a segment's header, records and problems
    lanstash seg verify <file>                        check every record body
    sudo lanstash install [--config FILE] [--KEY VALUE]... [--no-start] [--force] [--web DIR]
        install (or upgrade) as the systemd service `lanstash`: binary in /usr/local/bin, configuration in
        /etc/lanstash/lanstash.conf (from --config and the flags), started now and at every boot;
        --web DIR: the built web UI (web/dist) put where `web_ui` says, served on admin_listen
    sudo lanstash uninstall [--purge]
        stop and remove the service and the binary; --purge also removes /etc/lanstash. Cached data is kept.
    lanstash doctor [--config FILE] [--KEY VALUE]...
        check the installation, configuration, store, ports and DNS (default config: /etc/lanstash/lanstash.conf)
    lanstash help | version

SIZE accepts K, M, G, T suffixes (powers of 1024).
Exit codes: 0 ok, 1 problems found, 2 usage error, 3 failure.";

/// Where the configuration comes from: an optional file, then flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigArgs {
    pub file: Option<PathBuf>,
    pub flags: Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Serve(ConfigArgs),
    Import {
        config: ConfigArgs,
        nginx_cache: PathBuf,
        dirs: Vec<String>,
        threads: usize,
        slice: u64,
    },
    StoreStats(ConfigArgs),
    StoreVerify {
        config: ConfigArgs,
        threads: usize,
        /// Start without asking (systemd, scripts).
        accept: bool,
        /// Read at most this many MB a second (none: as fast as the disks go).
        rate_mb_s: Option<u64>,
    },
    Stop {
        config: ConfigArgs,
        wait_secs: u64,
    },
    /// Start a prefill in the running lanstash and follow it.
    Prefill {
        config: ConfigArgs,
        platform: String,
        game: String,
        pre: bool,
        from_lancache: bool,
        detach: bool,
    },
    PrefillStop(ConfigArgs),
    /// Steam: sign in once, check the kept token, or forget it.
    Steam {
        config: ConfigArgs,
        action: SteamAction,
    },
    /// lanstash's own accounts.
    User {
        config: ConfigArgs,
        action: UserAction,
    },
    Install {
        config: ConfigArgs,
        no_start: bool,
        force: bool,
        /// The built web UI to put where `web_ui` says.
        web: Option<PathBuf>,
    },
    Uninstall {
        purge: bool,
    },
    Doctor(ConfigArgs),
    SegDump {
        file: PathBuf,
        verify: bool,
        limit: usize,
    },
    SegVerify {
        file: PathBuf,
    },
}

/// What `lanstash steam` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamAction {
    /// Sign in by QR code (the default) or, with `--password`, account and password.
    Login {
        password: bool,
    },
    Status,
    Logout,
}

/// What `lanstash user` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserAction {
    Add { name: String, admin: bool },
    Passwd { name: String },
    Role { name: String, admin: bool },
    Remove { name: String },
    List,
}

/// `lanstash user <action> [<name>] [--admin | admin|viewer] [config flags]`.
fn parse_user(rest: &[&str]) -> Result<Command, String> {
    let admin = rest.contains(&"--admin");
    let rest: Vec<&str> = rest.iter().copied().filter(|a| *a != "--admin").collect();
    let (action, flags) = match rest.as_slice() {
        ["add", name, flags @ ..] => (UserAction::Add { name: (*name).into(), admin }, flags),
        ["passwd", name, flags @ ..] => (UserAction::Passwd { name: (*name).into() }, flags),
        ["role", name, role @ ("admin" | "viewer"), flags @ ..] => {
            (UserAction::Role { name: (*name).into(), admin: *role == "admin" }, flags)
        }
        ["role", ..] => return Err("user role <name> admin|viewer".into()),
        ["remove", name, flags @ ..] => (UserAction::Remove { name: (*name).into() }, flags),
        ["list", flags @ ..] => (UserAction::List, flags),
        _ => {
            return Err(
                "user: add <name> [--admin], passwd <name>, role <name> admin|viewer, remove <name>, list".into()
            );
        }
    };
    if admin && !matches!(action, UserAction::Add { .. }) {
        return Err("--admin belongs to user add".into());
    }
    if flags.first().is_some_and(|f| !f.starts_with("--")) {
        return Err(format!("unexpected argument {:?}", flags[0]));
    }
    Ok(Command::User { action, config: config_args(flags, &mut |_, _| Ok(false))? })
}

/// `lanstash steam <action> [--password] [config flags]`.
fn parse_steam(action: &str, rest: &[&str]) -> Result<Command, String> {
    let password = rest.contains(&"--password");
    let rest: Vec<&str> = rest.iter().copied().filter(|a| *a != "--password").collect();
    let action = match action {
        "login" => SteamAction::Login { password },
        "status" | "logout" if password => return Err("--password belongs to steam login".into()),
        "status" => SteamAction::Status,
        "logout" => SteamAction::Logout,
        other => return Err(format!("steam: login, status or logout, not {other:?}")),
    };
    Ok(Command::Steam { action, config: config_args(&rest, &mut |_, _| Ok(false))? })
}

/// Parses arguments (without the program name).
///
/// # Errors
/// A message to print on invalid usage.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        [] | ["help" | "--help" | "-h"] => Ok(Command::Help),
        ["version" | "--version"] => Ok(Command::Version),
        ["serve", rest @ ..] => Ok(Command::Serve(config_args(rest, &mut |_, _| Ok(false))?)),
        ["store", "stats", rest @ ..] => Ok(Command::StoreStats(config_args(rest, &mut |_, _| Ok(false))?)),
        ["store", "verify", rest @ ..] => parse_store_verify(rest),
        ["stop", rest @ ..] => parse_stop(rest),
        ["prefill", "stop", rest @ ..] => Ok(Command::PrefillStop(config_args(rest, &mut |_, _| Ok(false))?)),
        ["prefill", target, rest @ ..] => parse_prefill(target, rest),
        ["steam", action, rest @ ..] => parse_steam(action, rest),
        ["user", rest @ ..] => parse_user(rest),
        ["install", rest @ ..] => parse_install(rest),
        ["uninstall"] => Ok(Command::Uninstall { purge: false }),
        ["uninstall", "--purge"] => Ok(Command::Uninstall { purge: true }),
        ["doctor", rest @ ..] => Ok(Command::Doctor(config_args(rest, &mut |_, _| Ok(false))?)),
        ["import", rest @ ..] => parse_import(rest),
        ["seg", "dump", file, rest @ ..] => {
            let (mut verify, mut limit) = (false, 20);
            let mut it = rest.iter();
            while let Some(&flag) = it.next() {
                match flag {
                    "--verify" => verify = true,
                    "--limit" => limit = number(it.next(), flag)?,
                    other => return Err(format!("unknown option for seg dump: {other}")),
                }
            }
            Ok(Command::SegDump { file: PathBuf::from(file), verify, limit })
        }
        ["seg", "verify", file] => Ok(Command::SegVerify { file: PathBuf::from(file) }),
        _ => Err(format!("unrecognised command: {}", a.join(" "))),
    }
}

/// A command's own flags: given (key, value), whether it took them.
type OwnFlags<'a> = dyn FnMut(&str, Option<&str>) -> Result<bool, String> + 'a;

/// Collects `--config FILE` and `--KEY VALUE` flags. `own` gets the first
/// look at each flag (with its value) and returns whether it consumed it.
fn config_args(rest: &[&str], own: &mut OwnFlags<'_>) -> Result<ConfigArgs, String> {
    let mut c = ConfigArgs::default();
    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i];
        let next = rest.get(i + 1).copied();
        let Some(key) = flag.strip_prefix("--") else { return Err(format!("unexpected argument {flag:?}")) };
        if own(key, next)? {
            i += 2;
            continue;
        }
        if key == "config" {
            c.file = Some(next.ok_or("--config needs a file")?.into());
            i += 2;
        } else if is_switch(&key.replace('-', "_")) {
            c.flags.set(key, "true")?;
            i += 1;
        } else {
            let v = next.filter(|v| !v.starts_with("--")).ok_or_else(|| format!("--{key} needs a value"))?;
            c.flags.set(key, v).map_err(|e| format!("--{key}: {e}"))?;
            i += 2;
        }
    }
    Ok(c)
}

fn parse_import(rest: &[&str]) -> Result<Command, String> {
    let (mut nginx_cache, mut dirs, mut threads, mut slice) = (None, None, 16usize, 1u64 << 20);
    let config = config_args(rest, &mut |key, v| {
        let v = || v.ok_or_else(|| format!("--{key} needs a value"));
        match key {
            "nginx-cache" => nginx_cache = Some(PathBuf::from(v()?)),
            "dirs" => dirs = Some(parse_dirs(v()?)?),
            "threads" => threads = v()?.parse().ok().filter(|n| *n > 0).ok_or("--threads: a number ≥ 1")?,
            "slice" => slice = crate::config::values::size(v()?).filter(|n| *n > 0).ok_or("--slice: not a size")?,
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(Command::Import {
        config,
        nginx_cache: nginx_cache.ok_or("import needs --nginx-cache")?,
        dirs: dirs.ok_or("import needs --dirs")?,
        threads,
        slice,
    })
}

/// `00,1a,30-3f` → level directory names.
fn parse_dirs(spec: &str) -> Result<Vec<String>, String> {
    let hex = |s: &str| u8::from_str_radix(s, 16).ok().filter(|_| s.len() == 2);
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let (Some(a), Some(b)) = (hex(a), hex(b)) else {
            return Err(format!("--dirs: {part:?} is not a two-digit hex directory or range"));
        };
        if a > b {
            return Err(format!("--dirs: range {part:?} is backwards"));
        }
        out.extend((a..=b).map(|d| format!("{d:02x}")));
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err("--dirs is empty".into());
    }
    Ok(out)
}

fn number<T: std::str::FromStr>(v: Option<&&str>, flag: &str) -> Result<T, String> {
    let v = v.ok_or_else(|| format!("{flag} needs a value"))?;
    v.parse().map_err(|_| format!("{flag}: not a valid number: {v}"))
}

fn parse_store_verify(rest: &[&str]) -> Result<Command, String> {
    let (mut threads, mut rate_mb_s) = (4usize, None);
    let accept = rest.contains(&"--accept");
    let rest: Vec<&str> = rest.iter().copied().filter(|a| *a != "--accept").collect();
    let config = config_args(&rest, &mut |key, v| {
        match key {
            "threads" => {
                threads = v.and_then(|v| v.parse().ok()).filter(|n| *n > 0).ok_or("--threads: a number ≥ 1")?;
            }
            "rate" => {
                let mb = v.and_then(|v| v.parse::<u64>().ok()).filter(|n| *n > 0);
                rate_mb_s = Some(mb.ok_or("--rate: MB a second, a number ≥ 1")?);
            }
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(Command::StoreVerify { config, threads, accept, rate_mb_s })
}

fn parse_stop(rest: &[&str]) -> Result<Command, String> {
    let mut wait_secs = 60u64;
    let config = config_args(rest, &mut |key, v| {
        if key != "wait" {
            return Ok(false);
        }
        wait_secs = v.and_then(|v| v.parse().ok()).ok_or("--wait: seconds")?;
        Ok(true)
    })?;
    Ok(Command::Stop { config, wait_secs })
}

/// `<platform>/<game>`, then `--pre`, `--from lancache|cdn`, `--detach` and configuration flags.
fn parse_prefill(target: &str, rest: &[&str]) -> Result<Command, String> {
    let name = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let (platform, game) = target
        .split_once('/')
        .filter(|(p, g)| name(p) && name(g))
        .ok_or_else(|| format!("prefill: expected <platform>/<game>, got {target:?}"))?;
    // Switches of prefill itself; everything else is configuration.
    let (pre, detach) = (rest.contains(&"--pre"), rest.contains(&"--detach"));
    let rest: Vec<&str> = rest.iter().copied().filter(|a| !matches!(*a, "--pre" | "--detach")).collect();
    let mut from_lancache = false;
    let config = config_args(&rest, &mut |key, v| {
        if key != "from" {
            return Ok(false);
        }
        from_lancache = match v {
            Some("lancache") => true,
            Some("cdn") => false,
            _ => return Err("--from: lancache or cdn".into()),
        };
        Ok(true)
    })?;
    Ok(Command::Prefill { config, platform: platform.into(), game: game.into(), pre, from_lancache, detach })
}

fn parse_install(rest: &[&str]) -> Result<Command, String> {
    // Switches of install itself; everything else is configuration.
    let no_start = rest.contains(&"--no-start");
    let force = rest.contains(&"--force");
    let rest: Vec<&str> = rest.iter().copied().filter(|a| *a != "--no-start" && *a != "--force").collect();
    let mut web = None;
    let config = config_args(&rest, &mut |k, v| {
        if k != "web" {
            return Ok(false);
        }
        web = Some(PathBuf::from(v.ok_or("--web: the built web UI's directory (web/dist)")?));
        Ok(true)
    })?;
    Ok(Command::Install { config, no_start, force, web })
}
