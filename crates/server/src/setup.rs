//! `lanstash install`, `uninstall` and `doctor`: the steps that touch the
//! system (files under /etc and /usr/local, systemd). The texts and the
//! checklist format are in `lanstash::install`.

use std::fs;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lanstash::cli::ConfigArgs;
use lanstash::config::{Config, Settings};
use lanstash::install::{Layout, installed_config, render_checks, render_unit};
use lanstash::report::Outcome;
use lanstash_adapter_admin_api::client::{is_up, local_addr, status, store_figures};
use lanstash_adapter_dns::resolver::{Resolve, ResolverConfig, UdpResolver};
use lanstash_adapter_os::process::{Whose, open_files};
use lanstash_adapter_os::systemd::Systemd;
use lanstash_app::health::{self, Check, Level};
use lanstash_ports::clock::Clock;
use lanstash_ports::releases::Releases;
use lanstash_store::game::SegmentOptions;
use lanstash_store::store::{Store, StoreOptions};

use crate::run::{dir_fs, host_rules, store_roots};

fn io_err(what: &Path, e: &io::Error) -> String {
    let hint = if e.kind() == io::ErrorKind::PermissionDenied { " (run it with sudo)" } else { "" };
    format!("{}: {e}{hint}", what.display())
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| io_err(d, &e))?;
    }
    fs::write(path, text).map_err(|e| io_err(path, &e))
}

/// The addresses lanstash will listen on.
fn listen_addrs(c: &Config) -> Vec<SocketAddr> {
    std::iter::once(c.listen).chain(c.listen_tls).chain(c.admin_listen).collect()
}

/// The configuration text and its parsed form, for installing.
fn configuration(args: &ConfigArgs, l: &Layout) -> Result<(String, Config), String> {
    let source = match &args.file {
        Some(f) => Some(fs::read_to_string(f).map_err(|e| io_err(f, &e))?),
        None => fs::read_to_string(&l.config).ok(),
    };
    let flags: Vec<(&str, &str)> = args.flags.pairs().collect();
    let parse = |text: &str| {
        let mut s = Settings::default();
        s.add_file("lanstash.conf", text)?;
        s.build()
    };
    let first = parse(&installed_config(source.as_deref(), &flags, true))?;
    let text = installed_config(source.as_deref(), &flags, first.admin_listen.is_some());
    let c = parse(&text)?;
    if c.upstream_dns.is_empty() {
        return Err("`upstream_dns` is required (the DNS for origin names, not the one pointing at the cache)".into());
    }
    if let Some(p) = c.store.iter().chain(c.store_per_service.values().flatten()).find(|p| !p.is_absolute()) {
        return Err(format!("store path {} must be absolute", p.display()));
    }
    Ok((text, c))
}

/// Copies this program to `to` (via a temporary name and a rename, so a
/// running copy keeps its file).
fn install_binary(to: &Path) -> Result<bool, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if fs::canonicalize(&me).ok() == fs::canonicalize(to).ok() {
        return Ok(false);
    }
    let tmp = to.with_extension("new");
    if let Some(d) = to.parent() {
        fs::create_dir_all(d).map_err(|e| io_err(d, &e))?;
    }
    fs::copy(&me, &tmp).map_err(|e| io_err(&tmp, &e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).map_err(|e| io_err(&tmp, &e))?;
    }
    fs::rename(&tmp, to).map_err(|e| io_err(to, &e))?;
    Ok(true)
}

/// Puts the built web UI in `from` where `to` is: copied beside it, then swapped in whole.
fn install_web(from: &Path, to: &Path) -> Result<(), String> {
    let none = || format!("{}: no index.html (build it with `pnpm build` in web/)", from.display());
    from.join("index.html").is_file().then_some(()).ok_or_else(none)?;
    let (next, old) = (to.with_extension("new"), to.with_extension("old"));
    let _ = (fs::remove_dir_all(&next), fs::remove_dir_all(&old));
    copy_tree(from, &next)?;
    if to.exists() {
        fs::rename(to, &old).map_err(|e| io_err(to, &e))?;
    }
    fs::rename(&next, to).map_err(|e| io_err(to, &e))?;
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| io_err(to, &e))?;
    for entry in fs::read_dir(from).map_err(|e| io_err(from, &e))? {
        let entry = entry.map_err(|e| io_err(from, &e))?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().map_err(|e| io_err(&src, &e))?.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| io_err(&src, &e))?;
        }
    }
    Ok(())
}

pub fn install(args: &ConfigArgs, no_start: bool, force: bool, web: Option<&Path>) -> Result<Outcome, String> {
    let (l, sd) = (Layout::default(), Systemd::default());
    if !cfg!(target_os = "linux") || !sd.available() {
        return Err("install needs Linux with systemd".into());
    }
    let (text, c) = configuration(args, &l)?;
    if let (Some(from), Some(to)) = (web, c.web_ui.as_deref()) {
        install_web(from, to)?;
        println!("web UI         {}", to.display());
    }
    let state = sd.state(&l.unit);
    if !state.active && !force {
        for a in listen_addrs(&c) {
            if let Err(e) = TcpListener::bind(a) {
                return Err(format!(
                    "{a} is not free ({e}): is lancache or a test lanstash still running? Stop it, or use --force"
                ));
            }
        }
    }
    let mounts: Vec<PathBuf> = c.store.iter().chain(c.store_per_service.values().flatten()).cloned().collect();
    let unit = render_unit(&l, &mounts)?;

    if install_binary(&l.bin)? {
        println!("binary         {}", l.bin.display());
    }
    match fs::read_to_string(&l.config) {
        Ok(old) if old == text => println!("configuration  {} (unchanged)", l.config.display()),
        Ok(old) => {
            let bak = l.config.with_extension("conf.bak");
            write_file(&bak, &old)?;
            write_file(&l.config, &text)?;
            println!("configuration  {} (previous one in {})", l.config.display(), bak.display());
        }
        Err(_) => {
            write_file(&l.config, &text)?;
            println!("configuration  {}", l.config.display());
        }
    }
    for log in [&c.access_log, &c.event_log].into_iter().flatten() {
        if let Some(d) = log.parent() {
            fs::create_dir_all(d).map_err(|e| io_err(d, &e))?;
        }
    }
    write_file(&l.unit_file, &unit)?;
    println!("service        {}", l.unit_file.display());
    sd.daemon_reload()?;
    if no_start {
        println!("not started (--no-start): `systemctl enable --now {}` starts it", l.unit);
        return Ok(Outcome::Ok);
    }
    if state.active {
        sd.restart(&l.unit)?;
        println!("restarted      {}", l.unit);
    } else {
        sd.enable_now(&l.unit)?;
        println!("started        {} (and at every boot)", l.unit);
    }
    let admin = c.admin_listen.map(local_addr).ok_or("admin_listen missing")?;
    let started = Instant::now();
    while !is_up(admin, Duration::from_millis(500)) {
        if started.elapsed() > Duration::from_secs(60) {
            return Err(format!("started, but {admin} does not answer after 60 s: see `journalctl -u {}`", l.unit));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    println!("running        status page http://{}/ (logs: journalctl -u {})", c.admin_listen.unwrap_or(admin), l.unit);
    Ok(Outcome::Ok)
}

pub fn uninstall(purge: bool) -> Result<Outcome, String> {
    let (l, sd) = (Layout::default(), Systemd::default());
    let stores: Vec<PathBuf> = fs::read_to_string(&l.config)
        .ok()
        .and_then(|t| {
            let mut s = Settings::default();
            s.add_file("lanstash.conf", &t).ok()?;
            s.build().ok()
        })
        .map(|c| c.store.iter().chain(c.store_per_service.values().flatten()).cloned().collect())
        .unwrap_or_default();
    if sd.available() && sd.state(&l.unit).installed {
        sd.disable_now(&l.unit)?; // runs `lanstash stop`: a clean stop
        println!("stopped and disabled {}", l.unit);
    }
    if l.unit_file.exists() {
        fs::remove_file(&l.unit_file).map_err(|e| io_err(&l.unit_file, &e))?;
        println!("removed {}", l.unit_file.display());
        if sd.available() {
            sd.daemon_reload()?;
        }
    }
    if l.bin.exists() {
        fs::remove_file(&l.bin).map_err(|e| io_err(&l.bin, &e))?;
        println!("removed {}", l.bin.display());
    }
    match l.config.parent() {
        Some(d) if purge && d.exists() => {
            fs::remove_dir_all(d).map_err(|e| io_err(d, &e))?;
            println!("removed {}", d.display());
        }
        Some(d) if d.exists() => println!("kept    {} (remove it with --purge)", d.display()),
        _ => {}
    }
    for s in stores {
        println!("kept    {} (cached data; delete it yourself if you want the space back)", s.display());
    }
    Ok(Outcome::Ok)
}

/// `lanstash doctor`: checks the installation, the configuration and what
/// they depend on. Read-only.
pub fn doctor(args: &ConfigArgs, clock: &Arc<dyn Clock>, releases: &dyn Releases) -> Result<Outcome, String> {
    let (l, sd) = (Layout::default(), Systemd::default());
    let mut checks = Vec::new();
    macro_rules! add {
        ($level:expr, $what:expr, $detail:expr $(,)?) => {
            checks.push(Check::new($level, $what, $detail))
        };
    }

    let version = env!("CARGO_PKG_VERSION");
    match releases.latest() {
        Ok(Some(v)) if v != version => add!(Level::Warn, "version", format!("{version}; {v} is available")),
        Ok(Some(_)) => add!(Level::Ok, "version", format!("{version} (the latest)")),
        Ok(None) => add!(Level::Ok, "version", format!("{version} (newer releases: not checked yet)")),
        Err(e) => add!(Level::Warn, "version", format!("{version} (could not check for newer: {e})")),
    }

    let systemd = sd.available();
    let state = systemd.then(|| sd.state(&l.unit));
    match state {
        None => add!(Level::Warn, "service", "no systemd here"),
        Some(s) if !s.installed => {
            add!(Level::Warn, "service", "not installed (`sudo lanstash install --config FILE`)")
        }
        Some(s) => add!(
            if s.enabled && s.active { Level::Ok } else { Level::Warn },
            "service",
            format!(
                "{}: {}, {}",
                l.unit,
                if s.enabled { "enabled" } else { "not enabled" },
                if s.active { "running" } else { "not running" }
            ),
        ),
    }
    let running = state.is_some_and(|s| s.active);

    let file = args.file.clone().or_else(|| l.config.exists().then(|| l.config.clone()));
    let args = ConfigArgs { file: file.clone(), flags: args.flags.clone() };
    let c = match crate::run::config(&args) {
        Ok(c) => {
            add!(Level::Ok, "configuration", file.map_or_else(|| "from flags".into(), |f| f.display().to_string()));
            c
        }
        Err(e) => {
            add!(Level::Fail, "configuration", e);
            print!("{}", render_checks(&checks));
            return Ok(Outcome::ProblemsFound);
        }
    };
    if c.admin_listen.is_none() {
        add!(Level::Warn, "admin_listen", "not set: no status page, and `lanstash stop` can't stop it cleanly");
    }

    // A lanstash answering on the admin port while the service is not running: a test run.
    let other_lanstash = !running
        && c.admin_listen.is_some_and(|a| {
            status(local_addr(a), Duration::from_secs(2)).is_ok_and(|(s, b)| s == 200 && b.contains("\"counters\""))
        });
    for a in listen_addrs(&c) {
        let what = format!("port {}", a.port());
        if running {
            let up = is_up(local_addr(a), Duration::from_secs(2));
            add!(
                if up { Level::Ok } else { Level::Fail },
                &what,
                if up { format!("{a}: answering") } else { format!("{a}: not answering") }
            );
        } else {
            match TcpListener::bind(a) {
                Ok(_) => add!(Level::Ok, &what, format!("{a}: free")),
                Err(_) if other_lanstash => {
                    add!(Level::Warn, &what, format!("{a}: held by a lanstash outside the service"))
                }
                Err(e) => add!(Level::Fail, &what, format!("{a}: in use by something else ({e})")),
            }
        }
    }
    // The running service's own figures: what it holds, as it serves it (no second load of the store).
    let mut served: Option<String> = None;
    if running && let Some(a) = c.admin_listen {
        match status(local_addr(a), Duration::from_secs(5)) {
            Ok((200, body)) => {
                add!(Level::Ok, "admin API", format!("http://{a}/ (API: /api/v1)"));
                served = Some(body);
            }
            Ok((s, _)) => add!(Level::Fail, "admin API", format!("answered {s}")),
            Err(e) => add!(Level::Fail, "admin API", e),
        }
    }
    if let Some(pid) = state.and_then(|s| s.main_pid) {
        match open_files(Whose::Pid(pid)) {
            Ok((open, limit)) => checks.push(health::open_files("the service", open, limit)),
            Err(e) => add!(Level::Warn, "open files", format!("the service's: {e} (run doctor with sudo)")),
        }
    }

    let roots: Vec<&PathBuf> =
        c.store.iter().take(1).chain(c.store_per_service.values().filter_map(|p| p.first())).collect();
    for root in &roots {
        let probe = root.join(format!(".lanstash-doctor-{}", std::process::id()));
        match fs::write(&probe, b"x").and_then(|()| fs::remove_file(&probe)) {
            Ok(()) => add!(Level::Ok, "store", format!("{}: writable", root.display())),
            Err(e) => add!(Level::Fail, "store", io_err(root, &e)),
        }
        match dir_fs(std::slice::from_ref(root)).and_then(|fs| fs.space().map_err(|e| e.to_string())) {
            Ok(Some(d)) => checks.push(health::free_space(d.free, d.total, c.keep_free)),
            Ok(None) => add!(Level::Warn, "free space", "unknown"),
            Err(e) => add!(Level::Warn, "free space", e),
        }
    }
    match served.as_deref().and_then(store_figures) {
        // The service holds the store: its figures, and what its background check found.
        Some(f) => {
            let mut detail = format!("{} games, {:.2} GB (as the service holds them)", f.games, f.bytes as f64 / 1e9);
            if f.problems > 0 {
                detail.push_str(&format!("; its background check found {} problems", f.problems));
            }
            add!(if f.problems > 0 { Level::Warn } else { Level::Ok }, "store contents", detail);
        }
        // No service: the store is loaded here, read-only.
        None => {
            let opened = store_roots(&c).and_then(|roots| {
                let opts = StoreOptions {
                    segments: SegmentOptions { max_bytes: c.segment_size, ..SegmentOptions::default() },
                    read_only: true,
                };
                Store::open(roots, opts, Arc::clone(clock)).map_err(|e| e.to_string())
            });
            match opened {
                Ok((store, r)) => {
                    let t = &r.totals;
                    let limit = open_files(Whose::This).ok().map(|(_, l)| l);
                    let damaged = t.bad_bodies + t.journal_bad;
                    checks.push(health::store_load(r.games, store.stored_bytes(), &t.failed, damaged, limit));
                }
                Err(e) => add!(Level::Fail, "store contents", e),
            }
        }
    }
    if c.upstream_dns.is_empty() {
        add!(Level::Fail, "upstream DNS", "`upstream_dns` is not set");
    } else {
        let r = UdpResolver::new(
            ResolverConfig { servers: c.upstream_dns.clone(), ..ResolverConfig::default() },
            Arc::clone(clock),
        );
        match r.resolve("us.patch.battle.net") {
            Ok(ips) if !ips.is_empty() => add!(Level::Ok, "upstream DNS", format!("us.patch.battle.net → {}", ips[0])),
            Ok(_) => add!(Level::Fail, "upstream DNS", "no address for us.patch.battle.net"),
            Err(e) => add!(Level::Fail, "upstream DNS", e),
        }
    }
    match host_rules(&c) {
        Ok((_, counts)) => {
            let list: Vec<String> = counts.iter().map(|(s, n)| format!("{s} ({n})")).collect();
            add!(Level::Ok, "cache domains", format!("host patterns: {}", list.join(", ")))
        }
        Err(e) => add!(Level::Fail, "cache domains", e),
    }
    for log in [&c.access_log, &c.event_log].into_iter().flatten() {
        match log.parent() {
            Some(d) if d.as_os_str().is_empty() || d.is_dir() => add!(Level::Ok, "log", log.display().to_string()),
            _ => add!(Level::Fail, "log", format!("{}: directory missing", log.display())),
        }
    }

    print!("{}", render_checks(&checks));
    Ok(if checks.iter().any(|c| c.level == Level::Fail) { Outcome::ProblemsFound } else { Outcome::Ok })
}
