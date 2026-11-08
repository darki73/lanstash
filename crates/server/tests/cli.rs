//! Command-line parsing.

use std::path::PathBuf;

use lanstash::cli::{Command, ConfigArgs, SteamAction, UserAction, parse};

fn p(s: &str) -> Result<Command, String> {
    parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
}

#[test]
fn simple_commands() {
    assert_eq!(p(""), Ok(Command::Help));
    assert_eq!(p("--version"), Ok(Command::Version));
    assert_eq!(
        p("seg dump a.lcseg --verify --limit 0"),
        Ok(Command::SegDump { file: "a.lcseg".into(), verify: true, limit: 0 })
    );
    assert_eq!(p("seg verify x"), Ok(Command::SegVerify { file: "x".into() }));
    assert!(p("frobnicate").is_err());
}

#[test]
fn serve_takes_config_and_key_flags() {
    let Ok(Command::Serve(c)) =
        p("serve --config /etc/lanstash.conf --store /mnt/s --access-log-headers --fetch-ahead 8")
    else {
        panic!()
    };
    assert_eq!(c.file, Some(PathBuf::from("/etc/lanstash.conf")));
    let pairs: Vec<_> = c.flags.pairs().collect();
    assert_eq!(pairs, vec![("store", "/mnt/s"), ("access_log_headers", "true"), ("fetch_ahead", "8")]);
    assert!(p("serve --bogus 1").unwrap_err().contains("bogus"));
    assert!(p("serve --store").unwrap_err().contains("needs a value"));
    assert!(p("serve --store --listen x").unwrap_err().contains("needs a value"));
}

#[test]
fn import_options() {
    let Ok(Command::Import { config, nginx_cache, dirs, threads, slice }) =
        p("import --nginx-cache /c --dirs 00-02,0a --threads 4 --store /s")
    else {
        panic!()
    };
    assert_eq!((nginx_cache, threads, slice), (PathBuf::from("/c"), 4, 1 << 20));
    assert_eq!(dirs, vec!["00", "01", "02", "0a"]);
    assert_eq!(config.flags.pairs().collect::<Vec<_>>(), vec![("store", "/s")]);
    assert!(p("import --dirs 00").unwrap_err().contains("--nginx-cache"));
    assert!(p("import --nginx-cache /c --dirs 0g").is_err());
    assert!(p("import --nginx-cache /c --dirs 05-01").is_err());
    assert!(p("import --nginx-cache /c --dirs 00 --threads 0").is_err());
}

#[test]
fn store_verify_options() {
    assert_eq!(
        p("store verify --config c.conf"),
        Ok(Command::StoreVerify {
            config: lanstash::cli::ConfigArgs { file: Some("c.conf".into()), ..Default::default() },
            threads: 4,
            accept: false,
            rate_mb_s: None,
        })
    );
    let Ok(Command::StoreVerify { config, threads, accept, rate_mb_s }) =
        p("store verify --threads 8 --accept --rate 200 --store /s")
    else {
        panic!()
    };
    assert_eq!((threads, accept, rate_mb_s), (8, true, Some(200)), "--accept: does not ask; --rate: MB a second");
    assert_eq!(config.flags.pairs().collect::<Vec<_>>(), vec![("store", "/s")]);
    assert!(p("store verify --threads 0").is_err());
    assert!(p("store verify --rate 0").is_err());
    assert!(p("store verify --rate fast").is_err());
}

#[test]
fn stop_options() {
    let Ok(Command::Stop { config, wait_secs }) = p("stop --config /etc/lanstash/lanstash.conf") else { panic!() };
    assert_eq!((config.file, wait_secs), (Some(PathBuf::from("/etc/lanstash/lanstash.conf")), 60));
    let Ok(Command::Stop { wait_secs, .. }) = p("stop --wait 5") else { panic!() };
    assert_eq!(wait_secs, 5);
    assert!(p("stop --wait soon").is_err());
}

#[test]
fn prefill_options() {
    let Ok(Command::Prefill { config, platform, game, pre, from_lancache, detach }) =
        p("prefill blizzard/ovw --from lancache --config /etc/lanstash/lanstash.conf")
    else {
        panic!()
    };
    assert_eq!((platform.as_str(), game.as_str(), pre, from_lancache, detach), ("blizzard", "ovw", false, true, false));
    assert_eq!(config.file, Some(PathBuf::from("/etc/lanstash/lanstash.conf")));
    let Ok(Command::Prefill { pre, from_lancache, detach, .. }) = p("prefill blizzard/osi --pre --detach --from cdn")
    else {
        panic!()
    };
    assert_eq!((pre, from_lancache, detach), (true, false, true));
    assert!(matches!(p("prefill stop"), Ok(Command::PrefillStop(_))));
    assert!(p("prefill ovw").is_err(), "the platform is required");
    assert!(p("prefill blizzard/../x").is_err());
    assert!(p("prefill blizzard/ovw --from disk").is_err());
}

#[test]
fn install_uninstall_doctor() {
    let Ok(Command::Install { config, no_start, force, web }) = p("install --config c.conf --no-start --max-size 2T")
    else {
        panic!()
    };
    assert_eq!((config.file, no_start, force, web), (Some(PathBuf::from("c.conf")), true, false, None));
    assert_eq!(config.flags.pairs().collect::<Vec<_>>(), vec![("max_size", "2T")]);
    let Ok(Command::Install { config, web, .. }) = p("install --web web/dist --admin-listen 0.0.0.0:8081") else {
        panic!()
    };
    assert_eq!(web, Some(PathBuf::from("web/dist")), "the built web UI, not a setting");
    assert_eq!(config.flags.pairs().collect::<Vec<_>>(), vec![("admin_listen", "0.0.0.0:8081")]);
    assert!(p("install --web").is_err());
    assert_eq!(p("uninstall"), Ok(Command::Uninstall { purge: false }));
    assert_eq!(p("uninstall --purge"), Ok(Command::Uninstall { purge: true }));
    assert!(p("uninstall --everything").is_err());
    assert!(matches!(p("doctor"), Ok(Command::Doctor(_))));
}

#[test]
fn user_commands() {
    let plain = |action| Ok(Command::User { config: ConfigArgs::default(), action });
    let name = || "player.one".to_string();
    assert_eq!(p("user add player.one"), plain(UserAction::Add { name: name(), admin: false }));
    assert_eq!(p("user add player.one --admin"), plain(UserAction::Add { name: name(), admin: true }));
    assert_eq!(p("user passwd player.one"), plain(UserAction::Passwd { name: name() }));
    assert_eq!(p("user role player.one viewer"), plain(UserAction::Role { name: name(), admin: false }));
    assert_eq!(p("user remove player.one"), plain(UserAction::Remove { name: name() }));
    assert_eq!(p("user list"), plain(UserAction::List));
    let Ok(Command::User { config, .. }) = p("user list --config /etc/x.conf") else { panic!() };
    assert_eq!(config.file, Some(PathBuf::from("/etc/x.conf")));
    for bad in ["user", "user add", "user role player.one boss", "user remove player.one --admin", "user list extra"] {
        assert!(p(bad).is_err(), "{bad}");
    }
}

#[test]
fn steam_commands() {
    let plain = |action| Ok(Command::Steam { config: ConfigArgs::default(), action });
    assert_eq!(p("steam login"), plain(SteamAction::Login { password: false }));
    assert_eq!(p("steam login --password"), plain(SteamAction::Login { password: true }));
    assert!(p("steam status --password").is_err());
    assert_eq!(p("steam status"), plain(SteamAction::Status));
    assert_eq!(p("steam logout"), plain(SteamAction::Logout));
    let Ok(Command::Steam { config, .. }) = p("steam login --config /etc/x.conf") else { panic!() };
    assert_eq!(config.file, Some(PathBuf::from("/etc/x.conf")));
    assert_eq!(p("steam dance"), Err("steam: login, status or logout, not \"dance\"".into()));
}
