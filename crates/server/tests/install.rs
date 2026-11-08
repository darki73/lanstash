//! `lanstash::install`: the unit file, the installed configuration, the checklist.

use std::path::PathBuf;

use lanstash::config::Settings;
use lanstash::install::*;
use lanstash_app::health::{Check, Level};

#[test]
fn the_unit_waits_for_the_store_and_stops_cleanly() {
    let u = render_unit(&Layout::default(), &[PathBuf::from("/srv/stash")]).unwrap();
    for line in [
        "RequiresMountsFor=/srv/stash",
        "After=network-online.target remote-fs.target",
        "ExecStart=/usr/local/bin/lanstash serve --config /etc/lanstash/lanstash.conf",
        "ExecStop=/usr/local/bin/lanstash stop --config /etc/lanstash/lanstash.conf --wait 60",
        "TimeoutStopSec=90",
        "Restart=on-failure",
        "WantedBy=multi-user.target",
    ] {
        assert!(u.lines().any(|l| l == line), "missing {line:?} in\n{u}");
    }
    assert!(render_unit(&Layout::default(), &[PathBuf::from("/mnt/with space")]).is_err());
    assert!(render_unit(&Layout::default(), &[PathBuf::from("relative")]).is_err());
}

#[test]
fn the_installed_configuration_keeps_the_file_and_adds_flags_and_admin() {
    let t = installed_config(Some("store = /s\nupstream_dns = 1.1.1.1"), &[("max_size", "2T")], false);
    assert!(t.starts_with("store = /s\nupstream_dns = 1.1.1.1\n"));
    let mut s = Settings::default();
    s.add_file("x", &t).unwrap();
    let c = s.build().unwrap();
    assert_eq!(c.max_size, Some(2 << 40));
    assert_eq!(c.admin_listen, Some(DEFAULT_ADMIN.parse().unwrap()));
    // An admin address of its own is not overridden.
    assert!(!installed_config(Some("admin_listen = 0.0.0.0:8081\n"), &[], true).contains(DEFAULT_ADMIN));
}

#[test]
fn checklist() {
    let out = render_checks(&[
        Check::new(Level::Ok, "service", "running"),
        Check::new(Level::Warn, "free space", "low"),
        Check::new(Level::Fail, "port 80", "in use"),
    ]);
    assert!(out.contains("[ok  ] service") && out.contains("[WARN] free space") && out.contains("[FAIL] port 80"));
    assert!(out.ends_with("1 ok, 1 warnings, 1 failed\n"));
}
