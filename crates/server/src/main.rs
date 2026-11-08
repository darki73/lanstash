//! lanstash binary: the composition root. Parses arguments, builds adapters,
//! wires them into use cases and maps outcomes to exit codes. No other crate
//! reads arguments, environment or clocks.

#![forbid(unsafe_code)]

mod remote;
mod run;
mod setup;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use lanstash::cli::{self, Command};
use lanstash::report::Outcome;
use lanstash_ports::clock::Clock;
use lanstash_ports::releases::NoReleases;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match cli::parse(&args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("error: {msg}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let result = match command {
        Command::Help => {
            println!("{}", cli::USAGE);
            Ok(Outcome::Ok)
        }
        Command::Version => {
            println!("lanstash {}", env!("CARGO_PKG_VERSION"));
            Ok(Outcome::Ok)
        }
        Command::Serve(c) => run::serve::serve(&c, &clock),
        Command::Import { config, nginx_cache, dirs, threads, slice } => {
            run::store::import(&config, &nginx_cache, &dirs, threads, slice, &clock)
        }
        Command::StoreStats(c) => run::store::store_stats(&c, &clock),
        Command::StoreVerify { config, threads, accept, rate_mb_s } => {
            run::store::store_verify(&config, (threads, rate_mb_s), accept, &clock)
        }
        Command::Stop { config, wait_secs } => run::serve::stop(&config, std::time::Duration::from_secs(wait_secs)),
        Command::Prefill { config, platform, game, pre, from_lancache, detach } => {
            remote::prefill(&config, (&platform, &game), pre, from_lancache, detach)
        }
        Command::PrefillStop(c) => remote::prefill_stop(&c),
        Command::Steam { config, action } => run::steam::steam(&config, action),
        Command::User { config, action } => run::users::user(&config, &action),
        Command::Install { config, no_start, force, web } => setup::install(&config, no_start, force, web.as_deref()),
        Command::Uninstall { purge } => setup::uninstall(purge),
        Command::Doctor(c) => setup::doctor(&c, &clock, &NoReleases),
        Command::SegDump { file, verify, limit } => run::store::seg_dump(&file, verify, limit),
        Command::SegVerify { file } => run::store::seg_verify(&file),
    };
    match result {
        Ok(Outcome::Ok) => ExitCode::SUCCESS,
        Ok(Outcome::ProblemsFound) => ExitCode::from(1),
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(3)
        }
    }
}

/// The real wall clock, handed to adapters and use cases as a port.
struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
}
