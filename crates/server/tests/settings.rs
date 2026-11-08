//! Tests for `lanstash::settings`: the settings the web UI may change.

use std::path::PathBuf;

use lanstash::config::Settings;
use lanstash::settings::{LIVE, LiveSettings, read, saved};
use lanstash_ports::settings::Tunables;

/// A fresh directory with a configuration file, and the state directory in it.
fn place(name: &str, config: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("lanstash-settings-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("lanstash.conf");
    std::fs::write(&file, config).unwrap();
    (file, dir.join("state"))
}

#[test]
fn a_change_is_checked_kept_shown_and_dropped() {
    let (file, state) = place("change", "store = /mnt/a\nmax_size = 8000G\nkeep_free = 30%\n");
    let s = LiveSettings::new(Some(file), Settings::default(), &state);
    s.set("max_size", "6T").unwrap();
    assert_eq!(saved(&state), [("max_size".to_owned(), "6T".to_owned())]);
    let view = s.view();
    assert!(view.contains(r#"{"key":"max_size","group":"Storage","label":"Size limit","#), "{view}");
    assert!(view.contains(r#""value":"6T","config":"8000G","default":null,"ui":"6T","flag":false"#), "{view}");
    assert!(
        view.contains(r#""key":"steam_os","#)
            && view.contains(r#""value":"windows","config":null,"default":"windows","ui":null"#)
    );
    // Checked the way the configuration file is: nothing kept when refused.
    let e = s.set("steam_arch", "16").unwrap_err();
    assert!(e.contains("steam_arch"), "{e}");
    assert_eq!(saved(&state).len(), 1);
    assert!(s.set("listen", "0.0.0.0:81").unwrap_err().contains("configuration file only"));
    s.reset("max_size").unwrap();
    assert!(saved(&state).is_empty());
    assert!(s.view().contains(r#""value":"8000G","config":"8000G","default":null,"ui":null"#));
}

#[test]
fn a_flag_wins_and_cannot_be_changed_here() {
    let (file, state) = place("flag", "store = /mnt/a\n");
    let mut flags = Settings::default();
    flags.set("prefill_parallel", "4").unwrap();
    let s = LiveSettings::new(Some(file), flags, &state);
    assert!(s.set("prefill_parallel", "16").unwrap_err().contains("command-line flag"));
    assert!(
        s.view().contains(r#""key":"prefill_parallel","#)
            && s.view().contains(r#""value":"4","config":null,"default":"8","ui":null,"flag":true"#)
    );
}

#[test]
fn a_settings_file_gives_only_the_changeable_settings() {
    let got = read("max_size = 6T\n# a note\nlisten = 0.0.0.0:1\nsteam_os = linux # comment\n\nnonsense\n");
    assert_eq!(got, [("max_size".to_owned(), "6T".to_owned()), ("steam_os".to_owned(), "linux".to_owned())]);
    // Steam's language from a list; Battle.net's tags in parts; a window as its own value.
    let (file, state) = place("parts", "store = /mnt/a\n");
    let view = LiveSettings::new(Some(file), Settings::default(), &state).view();
    assert!(view.contains(r#""choices":["english","german","#), "{view}");
    assert!(view.contains(r#""tchinese"],"multi":true,"#), "several of Steam's languages: {view}");
    assert!(view.contains(r#""choices":["windows","linux","macos"],"multi":false,"#));
    assert!(view.contains(r#""parts":[{"label":"Platform","choices":["Windows","OSX"]},{"label":"Bitness","#));
    assert!(view.contains(r#""custom":{"label":"window","hint":"HH:MM-HH:MM, UTC"}"#));
    // Every one is a configuration key.
    let mut any = Settings::default();
    assert!(LIVE.iter().all(|l| any.set(l.key, "x").is_ok()));
}
