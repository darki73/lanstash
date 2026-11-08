//! `lanstash user add|passwd|role|remove|list`: lanstash's own accounts, wired to the terminal (a
//! password asked twice, with echo off), PBKDF2 and the secrets directory. Runs in the command's own
//! process (as root: the accounts stay root-only); the running service reads the change at once.
//! Also the service's sign-ins (`sessions`), wired from the same pieces.

use std::path::Path;
use std::sync::Arc;

use lanstash::cli::{ConfigArgs, UserAction};
use lanstash::config::Config;
use lanstash::report::Outcome;
use lanstash_adapter_crypto::digest::Sha256;
use lanstash_adapter_crypto::passwords::Pbkdf2;
use lanstash_adapter_crypto::random::SystemRng;
use lanstash_adapter_fs::private_dir::PrivateDir;
use lanstash_adapter_os::terminal::Terminal;
use lanstash_app::accounts::Accounts;
use lanstash_app::invites::Invites;
use lanstash_app::sessions::Sessions;
use lanstash_domain::accounts::Role;
use lanstash_ports::clock::Clock;
use lanstash_ports::prompt::Prompt;
use lanstash_ports::secrets::Secrets;

use super::config;

/// The installed configuration, used when no `--config` is given.
const INSTALLED: &str = "/etc/lanstash/lanstash.conf";

/// A new password, asked twice.
fn new_password(who: &str) -> Result<String, String> {
    let first = Terminal.ask(&format!("New password for {who}"), true)?;
    let again = Terminal.ask("The same again", true)?;
    if first != again {
        return Err("the two passwords differ: nothing changed".into());
    }
    Ok(first)
}

/// The service's sign-ins and invites: the accounts, sessions and invites kept in `secrets`.
pub(super) fn sessions(c: &Config, clock: &Arc<dyn Clock>) -> Result<(Arc<Sessions>, Arc<Invites>), String> {
    let secrets: Arc<dyn Secrets> = Arc::new(PrivateDir::new(&c.secrets));
    let accounts = Arc::new(Accounts::new(Arc::clone(&secrets), Arc::new(Pbkdf2::default()))?);
    if !accounts.has_admin() {
        println!("no admin account yet: `sudo lanstash user add <name> --admin` makes one");
    }
    let invites = Invites::new(
        Arc::clone(&accounts),
        Arc::clone(&secrets),
        Arc::new(SystemRng),
        Arc::new(Sha256),
        Arc::clone(clock),
    );
    let sessions = Sessions::new(accounts, secrets, Arc::new(SystemRng), Arc::new(Sha256), Arc::clone(clock))?;
    Ok((Arc::new(sessions), Arc::new(invites)))
}

const fn role(admin: bool) -> Role {
    if admin { Role::Admin } else { Role::Viewer }
}

pub fn user(args: &ConfigArgs, action: &UserAction) -> Result<Outcome, String> {
    let mut args = args.clone();
    if args.file.is_none() && Path::new(INSTALLED).exists() {
        args.file = Some(INSTALLED.into());
    }
    let c = config(&args)?;
    let accounts = Accounts::new(Arc::new(PrivateDir::new(&c.secrets)), Arc::new(Pbkdf2::default()))?;
    match action {
        UserAction::Add { name, admin } => {
            accounts.add(name, role(*admin), &new_password(name)?)?;
            println!("{name} added as {}.", role(*admin).name());
        }
        UserAction::Passwd { name } => {
            if !accounts.list().iter().any(|(n, _)| n == name) {
                return Err(format!("there is no account called {name}"));
            }
            accounts.set_password(name, &new_password(name)?)?;
            println!("{name}'s password changed.");
        }
        UserAction::Role { name, admin } => {
            accounts.set_role(name, role(*admin))?;
            println!("{name} is now {}.", role(*admin).name());
        }
        UserAction::Remove { name } => {
            accounts.remove(name)?;
            println!("{name} removed.");
        }
        UserAction::List => {
            let all = accounts.list();
            if all.is_empty() {
                println!("No accounts yet: `sudo lanstash user add <name> --admin` makes the first admin.");
            }
            for (name, r) in all {
                println!("{name:<32} {}", r.name());
            }
        }
    }
    Ok(Outcome::Ok)
}
