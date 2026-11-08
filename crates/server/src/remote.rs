//! Commands that only talk to the running lanstash through its admin port:
//! `lanstash prefill <platform>/<game>` starts a prefill there and follows
//! it (the job runs in the service and shows in its web UI; Ctrl-C only
//! stops following), `lanstash prefill stop` stops it.

use std::time::Duration;

use lanstash::cli::ConfigArgs;
use lanstash::prefill_cli::{progress, start_path};
use lanstash::report::Outcome;
use lanstash_adapter_admin_api::client;
use lanstash_domain::json;

use crate::run::config;

const TIMEOUT: Duration = Duration::from_secs(30);

fn admin(args: &ConfigArgs) -> Result<std::net::SocketAddr, String> {
    let c = config(args)?;
    let addr = c.admin_listen.ok_or("`admin_listen` is not set: the running lanstash is reached through it")?;
    Ok(client::local_addr(addr))
}

/// What the API said went wrong (its JSON error message, else the body).
fn reason(body: &str) -> String {
    json::parse(body)
        .ok()
        .and_then(|v| v.get("error").and_then(json::Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| body.trim().to_owned())
}

/// Starts a prefill, then prints its progress every `every` until it ends (unless `detach`).
pub fn prefill(
    args: &ConfigArgs,
    target: (&str, &str),
    prerelease: bool,
    from_lancache: bool,
    detach: bool,
) -> Result<Outcome, String> {
    let addr = admin(args)?;
    let (platform, game) = target;
    let (code, body) = client::request(addr, "POST", &start_path(platform, game, prerelease, from_lancache), TIMEOUT)?;
    if code != 202 {
        return Err(format!("prefill {platform}/{game} not started ({code}): {}", reason(&body)));
    }
    println!("prefill {platform}/{game} started; it runs in lanstash (and shows in its web UI)");
    if detach {
        return Ok(Outcome::Ok);
    }
    println!("following it (Ctrl-C stops following, not the prefill; `lanstash prefill stop` stops it)");
    let mut last = String::new();
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let (code, body) = client::request(addr, "GET", "/api/v1/prefill", TIMEOUT)?;
        let p = (code == 200).then(|| json::parse(&body).ok()).flatten().as_ref().and_then(progress);
        let Some(p) = p else { return Err(format!("no prefill status ({code}): {}", reason(&body))) };
        if p.line != last {
            println!("{}", p.line);
            last.clone_from(&p.line);
        }
        if p.game != game {
            return Err(format!("another prefill ({}) replaced it", p.game));
        }
        if !p.running() {
            return Ok(if p.state == "done" { Outcome::Ok } else { Outcome::ProblemsFound });
        }
    }
}

/// Stops the running prefill.
pub fn prefill_stop(args: &ConfigArgs) -> Result<Outcome, String> {
    let (code, body) = client::request(admin(args)?, "POST", "/api/v1/prefill/stop", TIMEOUT)?;
    if !(200..300).contains(&code) {
        return Err(format!("not stopped ({code}): {}", reason(&body)));
    }
    println!("prefill stopping");
    Ok(Outcome::Ok)
}
