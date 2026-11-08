//! Configuration: file, flags, defaults and validation.

use std::net::SocketAddr;
use std::path::PathBuf;

mod values;

use lanstash::config::Settings;
use lanstash::config::values::size;

fn build(file: &str, flags: &[(&str, &str)]) -> Result<lanstash::config::Config, String> {
    let mut s = Settings::default();
    s.add_file("test.conf", file)?;
    for (k, v) in flags {
        s.set(k, v)?;
    }
    s.build()
}

#[test]
fn defaults_file_and_flags() {
    let file = "\
        # lanstash\n\
        store = /mnt/a, /mnt/b   # two mounts of one share\n\
        store.blizzard = /mnt/fast\n\
        upstream_dns = 1.1.1.1, 9.9.9.9:5353\n\
        fetch_ahead = 2\n";
    let c = build(file, &[("fetch-ahead", "8"), ("access_log_headers", "true")]).unwrap();
    assert_eq!(c.listen_tls, None, "HTTPS pass-through is off by default");
    assert_eq!(c.listen, "0.0.0.0:80".parse::<SocketAddr>().unwrap());
    assert_eq!(c.store, vec![PathBuf::from("/mnt/a"), PathBuf::from("/mnt/b")]);
    assert_eq!(c.store_per_service["blizzard"], vec![PathBuf::from("/mnt/fast")]);
    assert_eq!(c.upstream_dns, vec!["1.1.1.1:53".parse().unwrap(), "9.9.9.9:5353".parse().unwrap()]);
    assert_eq!(c.fetch_ahead, 8, "flags override the file");
    assert!(c.access_log_headers && !c.prefetch);
    assert_eq!((c.fetch_piece, c.object_file_min, c.segment_size), (16 << 20, 16 << 20, 1 << 30));
    assert_eq!(c.services, vec!["steam", "blizzard", "epicgames", "riot"]);
}

#[test]
fn errors_name_the_line_or_key() {
    assert_eq!(build("store = /x\nbogus = 1\n", &[]).unwrap_err(), "test.conf:2: unknown setting `bogus`");
    assert_eq!(build("store /x\n", &[]).unwrap_err(), "test.conf:1: expected `key = value`");
    assert_eq!(build("", &[]).unwrap_err(), "`store` is required");
    assert!(build("store = /x\nfetch_ahead = many\n", &[]).unwrap_err().contains("fetch_ahead"));
    for ahead in ["0", "65", "100000"] {
        assert!(build(&format!("store = /x\nfetch_ahead = {ahead}\n"), &[]).unwrap_err().contains("fetch_ahead"));
    }
    assert!(build("store = /x\nfetch_ahead = 64\n", &[]).is_ok());
    assert!(build("store = /x\nupstream_dns = nope\n", &[]).unwrap_err().contains("upstream_dns"));
    assert!(build("store = /x\nservices = st/eam\n", &[]).unwrap_err().contains("services"));
    assert!(build("store = /x\nfetch_piece = 1K\n", &[]).is_err());
    assert!(build("store = /x\nlisten_tls = :443\n", &[]).unwrap_err().contains("listen_tls"));
    let tls = build("store = /x\nlisten_tls = 0.0.0.0:443\n", &[]).unwrap().listen_tls;
    assert_eq!(tls, Some("0.0.0.0:443".parse().unwrap()));
    assert!(Settings::default().set("store./x", "y").is_err());
}

#[test]
fn where_client_names_are_looked_up() {
    use lanstash::config::ClientNamesFrom;
    let from = |v: &str| build(&format!("store = /x\nclient_names = {v}\n"), &[]).map(|c| c.client_names);
    assert_eq!(build("store = /x\n", &[]).unwrap().client_names, ClientNamesFrom::System, "the default");
    assert_eq!(from("off"), Ok(ClientNamesFrom::Off));
    assert_eq!(
        from("10.0.0.1, 10.0.0.2:5353"),
        Ok(ClientNamesFrom::Servers(vec!["10.0.0.1:53".parse().unwrap(), "10.0.0.2:5353".parse().unwrap()]))
    );
    assert!(from("router").unwrap_err().contains("client_names"));
    assert!(from(",").is_err(), "no server at all");
}

#[test]
fn sizes() {
    assert_eq!(size("4096"), Some(4096));
    assert_eq!(size("512K"), Some(512 << 10));
    assert_eq!(size("16M"), Some(16 << 20));
    assert_eq!(size("1g"), Some(1 << 30));
    assert_eq!(size("x"), None);
    assert_eq!(size("99999999999999G"), None);
}

#[test]
fn eviction_limits() {
    use lanstash_domain::evict::KeepFree;
    let c = build("store = /s\n", &[]).unwrap();
    assert_eq!((c.max_size, c.keep_free, c.evict_secs), (None, Some(KeepFree::Percent(5)), 30));
    let c = build("store = /s\nmax_size = 2T\nkeep_free = 500G\n", &[]).unwrap();
    assert_eq!((c.max_size, c.keep_free), (Some(2 << 40), Some(KeepFree::Bytes(500 << 30))));
    let c = build("store = /s\nkeep_free = 0\nmax_size = 0\n", &[]).unwrap();
    assert_eq!((c.max_size, c.keep_free), (None, None));
    assert!(build("store = /s\nkeep_free = 100%\n", &[]).unwrap_err().contains("keep_free"));
    assert!(build("store = /s\nmax_size = lots\n", &[]).unwrap_err().contains("max_size"));
    assert!(build("store = /s\nevict_secs = 0\n", &[]).unwrap_err().contains("evict_secs"));
    assert_eq!(size("3t"), Some(3 << 40));
}

#[test]
fn log_rotation() {
    let c = build("store = /s\n", &[]).unwrap();
    assert_eq!((c.log_max_size, c.log_keep), (100 << 20, 5));
    let c = build("store = /s\nlog_max_size = 0\nlog_keep = 1\n", &[]).unwrap();
    assert_eq!((c.log_max_size, c.log_keep), (0, 1));
}

#[test]
fn battlenet_region_and_install_tags() {
    let c = build("store = /mnt/a\n", &[]).unwrap();
    assert_eq!(
        (c.blizzard_region.as_str(), c.blizzard_tags.clone()),
        ("us", vec!["Windows".into(), "x86_64".into(), "enUS".into()])
    );
    let c = build("store = /mnt/a\nblizzard_region = EU\nblizzard_tags = Windows, x86_64, deDE\n", &[]).unwrap();
    assert_eq!((c.blizzard_region.as_str(), c.blizzard_tags[2].as_str()), ("eu", "deDE"));
    assert!(build("store = /mnt/a\nblizzard_tags = Windows, ../x\n", &[]).is_err());
    assert!(build("store = /mnt/a\nblizzard_tags = ,\n", &[]).is_err());
    assert!(build("store = /mnt/a\nblizzard_region = europe\n", &[]).is_err());
    assert_eq!(build("store = /mnt/a\n", &[]).unwrap().prefill_parallel, 8);
    assert_eq!(build("store = /mnt/a\nprefill_parallel = 16\n", &[]).unwrap().prefill_parallel, 16);
    assert!(build("store = /mnt/a\nprefill_parallel = 0\n", &[]).is_err());
    assert!(build("store = /mnt/a\nprefill_parallel = 65\n", &[]).is_err());
}

#[test]
fn the_steam_target_defaults_to_windows_english_64_and_is_checked() {
    let c = build("store = /mnt/a\n", &[]).unwrap();
    assert_eq!((c.steam_os.as_str(), c.steam_language.as_str(), c.steam_arch.as_str()), ("windows", "english", "64"));
    let c = build("store = /mnt/a\nsteam_os = Linux\nsteam_language = German\nsteam_arch = 32\n", &[]).unwrap();
    assert_eq!((c.steam_os.as_str(), c.steam_language.as_str(), c.steam_arch.as_str()), ("linux", "german", "32"));
    assert!(build("store = /mnt/a\nsteam_os = dos\n", &[]).is_err());
    assert!(build("store = /mnt/a\nsteam_arch = 16\n", &[]).is_err());
    assert!(build("store = /mnt/a\nsteam_language = en-GB\n", &[]).is_err());
    let c = build("store = /mnt/a\nsteam_language = English, German\n", &[]).unwrap();
    assert_eq!(c.steam_language, "english,german", "several languages");
    assert!(build("store = /mnt/a\nsteam_language = english,\n", &[]).is_err(), "an empty one");
    assert_eq!(build("store = /mnt/a\n", &[]).unwrap().state, PathBuf::from("/var/lib/lanstash/state"));
}

#[test]
fn automatic_updates_are_off_unless_a_window_is_given() {
    assert_eq!(build("store = /mnt/a\n", &[]).unwrap().auto_update, None);
    assert_eq!(build("store = /mnt/a\nauto_update = off\n", &[]).unwrap().auto_update, None);
    let c = build("store = /mnt/a\nauto_update = 00:00-04:00\n", &[]).unwrap();
    assert_eq!(c.auto_update, lanstash_domain::window::Window::parse("00:00-04:00"));
    let always = build("store = /mnt/a\nauto_update = always\n", &[]).unwrap();
    assert_eq!(always.auto_update, lanstash_domain::window::Window::parse("00:00-24:00"));
    let e = build("store = /mnt/a\nauto_update = nightly\n", &[]).unwrap_err();
    assert!(e.contains("`auto_update`"), "{e}");
}

#[test]
fn the_background_store_check_is_off_unless_a_period_is_given() {
    let off = build("store = /mnt/a\n", &[]).unwrap();
    assert_eq!((off.verify_every_days, off.verify_rate), (None, 50 << 20));
    let every = |v: &str| build(&format!("store = /mnt/a\nverify_every = {v}\n"), &[]).map(|c| c.verify_every_days);
    assert_eq!((every("weekly"), every("monthly"), every("3d")), (Ok(Some(7)), Ok(Some(30)), Ok(Some(3))));
    assert!(every("0d").is_err() && every("daily").is_err() && every("400d").is_err());
    assert_eq!(build("store = /mnt/a\nverify_rate = 100M\n", &[]).unwrap().verify_rate, 100 << 20);
    assert!(build("store = /mnt/a\nverify_rate = 1K\n", &[]).is_err(), "too slow to finish");
}
