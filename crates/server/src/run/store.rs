//! The store commands: importing lancache's cache, statistics, and checking
//! segments and whole stores.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lanstash::cli::ConfigArgs;
use lanstash::report::{self, Outcome};
use lanstash_adapter_fs::fs_file::FsFile;
use lanstash_adapter_fs::nginx_dir::NginxDir;
use lanstash_adapter_os::pause::Sleep;
use lanstash_adapter_os::terminal::Terminal;
use lanstash_app::import_nginx::{ImportOptions, ImportReport, import_nginx};
use lanstash_app::verify::{Paced, VerifyProgress, verify_paced};
use lanstash_ports::clock::Clock;
use lanstash_ports::prompt::Prompt;
use lanstash_store::segment::scan::{ScanOptions, scan};

use super::platforms::{game_names, profiles};
use super::{config, open_store, store_roots};

/// Says what a full check reads and, unless `accept`, asks whether to start; whether to start.
/// No one to ask (no terminal, as under systemd) is no: `--accept` starts it unattended.
fn confirm(
    files: usize,
    bytes: u64,
    threads: usize,
    accept: bool,
    tell: &dyn Fn(&str),
    ask: &dyn Fn(&str) -> Result<String, String>,
) -> bool {
    tell(&format!(
        "This reads every stored byte: {files} segments and object files, {:.1} GB, with {threads} threads,\n\
         as fast as the disks go. On network storage it can fill the link (a NAS's other users and its own\n\
         web UI slow down) until it ends; lanstash keeps serving meanwhile, more slowly. It changes nothing.\n\
         To check slowly in the background instead, set `verify_every` (paced by `verify_rate`).",
        bytes as f64 / 1e9
    ));
    if accept {
        return true;
    }
    match ask("Start now? [y/N]") {
        Ok(a) if matches!(a.trim().to_lowercase().as_str(), "y" | "yes") => true,
        Ok(_) => {
            tell("Not started.");
            false
        }
        Err(_) => {
            tell("Not started: no one to ask. To run it unattended (from systemd, a script), add --accept.");
            false
        }
    }
}

pub fn import(
    args: &ConfigArgs,
    nginx_cache: &Path,
    dirs: &[String],
    threads: usize,
    slice: u64,
    clock: &Arc<dyn Clock>,
) -> Result<Outcome, String> {
    let c = config(args)?;
    let (store, _) = open_store(&c, clock, false)?;
    let opts = ImportOptions {
        top_dirs: dirs.to_vec(),
        threads,
        services: c.services.clone(),
        profiles: profiles(),
        slice_bytes: slice,
        object_file_min: c.object_file_min,
    };
    let started = Instant::now();
    eprintln!("listing {} level directories under {} …", dirs.len(), nginx_cache.display());
    let progress = |r: &ImportReport| {
        let secs = started.elapsed().as_secs_f64().max(0.001);
        eprintln!(
            "  {:>9}/{} files, {} records, {} extents, {:.1} GB, {:.0} MB/s",
            r.files_done,
            r.files_listed,
            r.records,
            r.extents,
            r.imported_bytes as f64 / 1e9,
            r.imported_bytes as f64 / 1e6 / secs
        );
    };
    let r = import_nginx(&NginxDir::new(nginx_cache), &store, &opts, &progress).map_err(|e| e.to_string())?;
    print!("{}", report::import(&r, started.elapsed().as_secs_f64()));
    Ok(if r.bad_files + r.check_failed == 0 { Outcome::Ok } else { Outcome::ProblemsFound })
}

pub fn store_stats(args: &ConfigArgs, clock: &Arc<dyn Clock>) -> Result<Outcome, String> {
    // Read-only: safe while a server runs on the same store.
    let c = config(args)?;
    let (store, _) = open_store(&c, clock, true)?;
    print!("{}", report::usage(&store.usage(), &*game_names(&c)?));
    Ok(Outcome::Ok)
}

pub fn seg_dump(file: &Path, verify: bool, limit: usize) -> Result<Outcome, String> {
    let seg = FsFile::open_read(file).map_err(|e| e.to_string())?;
    let rep = scan(&seg, ScanOptions { verify_bodies: verify }).map_err(|e| e.to_string())?;
    // A closed pipe (e.g. `| head`) is not an error for a listing tool.
    let text = report::segment(&file.display().to_string(), &rep, limit);
    if let Err(e) = std::io::Write::write_all(&mut std::io::stdout().lock(), text.as_bytes())
        && e.kind() != std::io::ErrorKind::BrokenPipe
    {
        return Err(e.to_string());
    }
    Ok(if rep.is_clean() { Outcome::Ok } else { Outcome::ProblemsFound })
}

pub fn seg_verify(file: &Path) -> Result<Outcome, String> {
    let seg = FsFile::open_read(file).map_err(|e| e.to_string())?;
    let rep = scan(&seg, ScanOptions { verify_bodies: true }).map_err(|e| e.to_string())?;
    let ok = rep.records.iter().filter(|r| r.body_ok == Some(true)).count();
    println!("{}: {} records, {} verified ok, {} problems", file.display(), rep.records.len(), ok, rep.problems.len());
    for p in &rep.problems {
        println!("  {p}");
    }
    Ok(if rep.is_clean() { Outcome::Ok } else { Outcome::ProblemsFound })
}

/// `pace`: the threads, and at most how many MB a second are read (none: as fast as the disks go).
pub fn store_verify(
    args: &ConfigArgs,
    pace: (usize, Option<u64>),
    accept: bool,
    clock: &Arc<dyn Clock>,
) -> Result<Outcome, String> {
    let (threads, rate_mb_s) = pace;
    // Read-only, like `store stats`: nothing is loaded into an index, every
    // segment and object file is read once.
    let c = config(args)?;
    let names = game_names(&c)?;
    let plan = lanstash_store::verify::plan(&store_roots(&c)?).map_err(|e| e.to_string())?;
    let bytes: u64 = plan.items.iter().map(|i| i.bytes).sum();
    // It reads as fast as the disks (and, for network storage, the link) go: say so, and ask.
    let ask = |q: &str| Terminal.ask(q, false);
    if !confirm(plan.items.len(), bytes, threads, accept, &|m: &str| Terminal.tell(m), &ask) {
        return Ok(Outcome::Ok);
    }
    let started = Instant::now();
    let last = std::sync::Mutex::new(Instant::now());
    let progress = |p: VerifyProgress| {
        let mut last = last.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if last.elapsed() < Duration::from_secs(5) && p.items_done < p.items {
            return;
        }
        *last = Instant::now();
        let secs = started.elapsed().as_secs_f64().max(0.001);
        eprintln!(
            "  {:>6}/{} files, {:.1}/{:.1} GB, {:.0} MB/s",
            p.items_done,
            p.items,
            p.bytes_done as f64 / 1e9,
            p.bytes as f64 / 1e9,
            p.bytes_done as f64 / 1e6 / secs
        );
    };
    match rate_mb_s {
        Some(mb) => eprintln!("checking {} segments and object files at most {mb} MB/s …", plan.items.len()),
        None => eprintln!("checking {} segments and object files with {threads} threads …", plan.items.len()),
    }
    let paced = rate_mb_s.map(|mb| Paced { bytes_per_s: mb * 1_000_000, clock: &**clock, pause: &Sleep });
    let r = verify_paced(plan, threads, paced.as_ref(), &progress);
    print!("{}", report::verify(&r, &*names, started.elapsed().as_secs_f64()));
    Ok(if r.is_clean() { Outcome::Ok } else { Outcome::ProblemsFound })
}
