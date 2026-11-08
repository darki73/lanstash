//! The settings the web UI may change. A value
//! counts in this order, later winning: the built-in default, the
//! configuration file, a change made in the UI (kept as `key = value` lines in
//! `<state>/settings`), a command-line flag. A change is checked the way the
//! configuration file is (the whole configuration is built with it) before it
//! is saved; it applies at the next start. Only the settings in [`LIVE`] can be
//! changed this way: where lanstash listens, what it stores where and the like
//! stay in the configuration file.

use std::path::{Path, PathBuf};

use lanstash_domain::json::string;
use lanstash_ports::settings::Tunables;

use crate::config::Settings;

/// A setting the UI may change, and how it presents it.
#[derive(Debug, Clone, Copy)]
pub struct Live {
    pub key: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    /// The values to pick from (a long list is offered as you type).
    pub choices: &'static [&'static str],
}

/// A choice besides the listed ones that takes a value of its own, typed in:
/// its label and what such a value looks like.
#[must_use]
pub fn custom(key: &str) -> Option<(&'static str, &'static str)> {
    match key {
        "auto_update" => Some(("window", "HH:MM-HH:MM, UTC")),
        "prefill_parallel" => Some(("other", "1 to 64")),
        "verify_every" => Some(("every", "<n>d, 1 to 365 days")),
        "verify_rate" => Some(("other", "a size a second, 1M or more")),
        _ => None,
    }
}

/// Whether the value is several of the choices, comma-separated (Steam's languages).
#[must_use]
pub fn multi(key: &str) -> bool {
    key == "steam_language"
}

/// For a comma-separated value: its parts, each picked from its list.
#[must_use]
pub fn parts(key: &str) -> &'static [Part] {
    if key == "blizzard_tags" { BLIZZARD_TAGS } else { &[] }
}

/// One part of a comma-separated value, and what it may be.
#[derive(Debug, Clone, Copy)]
pub struct Part {
    pub label: &'static str,
    pub choices: &'static [&'static str],
}

/// Steam's names of the languages it ships games in.
const STEAM_LANGUAGES: &[&str] = &[
    "english",
    "german",
    "french",
    "italian",
    "spanish",
    "latam",
    "portuguese",
    "brazilian",
    "russian",
    "polish",
    "dutch",
    "danish",
    "finnish",
    "norwegian",
    "swedish",
    "czech",
    "hungarian",
    "romanian",
    "bulgarian",
    "greek",
    "turkish",
    "ukrainian",
    "arabic",
    "thai",
    "vietnamese",
    "indonesian",
    "japanese",
    "koreana",
    "schinese",
    "tchinese",
];

/// The parts of a Battle.net install's tags: platform, bitness, language.
const BLIZZARD_TAGS: &[Part] = &[
    Part { label: "Platform", choices: &["Windows", "OSX"] },
    Part { label: "Bitness", choices: &["x86_64", "x86_32", "arm64"] },
    Part {
        label: "Language",
        choices: &[
            "enUS", "enGB", "deDE", "esES", "esMX", "frFR", "itIT", "koKR", "plPL", "ptBR", "ptPT", "ruRU", "zhCN",
            "zhTW", "jaJP",
        ],
    },
];

/// The settings the UI may change.
pub const LIVE: &[Live] = &[
    Live {
        key: "auto_update",
        group: "Updates",
        label: "Automatic updates",
        help: "Games once held whole are queued for their new builds: always, in a daily window (HH:MM-HH:MM, UTC), or off.",
        choices: &["always", "off"],
    },
    Live {
        key: "prefill_parallel",
        group: "Updates",
        label: "Prefill requests at once",
        help: "How many requests a prefill keeps in flight (1 to 64).",
        choices: &["4", "8", "16", "32"],
    },
    Live {
        key: "steam_os",
        group: "Steam",
        label: "Operating system",
        help: "Who a Steam install is for: prefills, builds and sizes follow it.",
        choices: &["windows", "linux", "macos"],
    },
    Live {
        key: "steam_language",
        group: "Steam",
        label: "Languages",
        help: "The languages your PCs play in: an install takes each one's files, so none is fetched from the internet.",
        choices: STEAM_LANGUAGES,
    },
    Live { key: "steam_arch", group: "Steam", label: "Architecture", help: "64 or 32 bit.", choices: &["64", "32"] },
    Live {
        key: "blizzard_region",
        group: "Battle.net",
        label: "Region",
        help: "The region whose current builds count.",
        choices: &["us", "eu", "kr", "tw", "cn"],
    },
    Live {
        key: "blizzard_tags",
        group: "Battle.net",
        label: "Install tags",
        help: "What a Battle.net install is (platform, architecture, locale, optional content), comma-separated.",
        choices: &[],
    },
    Live {
        key: "max_size",
        group: "Storage",
        label: "Size limit",
        help: "The most the stash may hold (8000G, 2T); eviction makes room beyond it. Empty: no limit.",
        choices: &[],
    },
    Live {
        key: "keep_free",
        group: "Storage",
        label: "Keep free",
        help: "Space eviction keeps free on the disk (30%, 500G).",
        choices: &[],
    },
    Live {
        key: "verify_every",
        group: "Storage",
        label: "Check stored bytes",
        help: "Reads every stored byte against its checksum in the background, paced, and reports damage: weekly, monthly, every <n>d, or off.",
        choices: &["off", "weekly", "monthly"],
    },
    Live {
        key: "verify_rate",
        group: "Storage",
        label: "Check speed",
        help: "The most the check reads a second (20M, 50M, 100M), so clients keep the disks.",
        choices: &["20M", "50M", "100M"],
    },
];

/// The file the changes are kept in, under the state directory.
pub const FILE: &str = "settings";

fn live(key: &str) -> Option<&'static Live> {
    LIVE.iter().find(|l| l.key == key)
}

/// The changes a settings file holds: `key = value` lines of [`LIVE`] keys
/// (anything else is left out).
#[must_use]
pub fn read(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.split('#').next()?.split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .filter(|(k, _)| live(k).is_some())
        .collect()
}

/// The changes kept in `state`'s settings file (none when there is none).
#[must_use]
pub fn saved(state: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(state.join(FILE)).map(|t| read(&t)).unwrap_or_default()
}

/// The UI's settings of a running lanstash.
#[derive(Debug, Clone)]
pub struct LiveSettings {
    /// The configuration file, if any.
    config: Option<PathBuf>,
    /// The command-line flags.
    flags: Settings,
    state: PathBuf,
}

impl LiveSettings {
    #[must_use]
    pub fn new(config: Option<PathBuf>, flags: Settings, state: &Path) -> Self {
        Self { config, flags, state: state.to_owned() }
    }

    fn file(&self) -> Result<Settings, String> {
        let mut s = Settings::default();
        if let Some(f) = &self.config {
            let text = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
            s.add_file(&f.display().to_string(), &text)?;
        }
        Ok(s)
    }

    /// The whole configuration with `changes`: the file, them, the flags.
    fn with(&self, changes: &[(String, String)]) -> Result<Settings, String> {
        let mut s = self.file()?;
        for (k, v) in changes {
            s.set(k, v)?;
        }
        for (k, v) in self.flags.pairs() {
            s.set(k, v)?;
        }
        Ok(s)
    }

    /// Checks `changes` (the whole configuration is built with them) and keeps them.
    fn save(&self, changes: &[(String, String)]) -> Result<(), String> {
        self.with(changes)?.build()?;
        let text: String = changes.iter().map(|(k, v)| format!("{k} = {v}\n")).collect();
        let (path, tmp) = (self.state.join(FILE), self.state.join(format!("{FILE}.tmp")));
        std::fs::create_dir_all(&self.state).map_err(|e| format!("{}: {e}", self.state.display()))?;
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn changeable(&self, key: &str) -> Result<(), String> {
        if live(key).is_none() {
            return Err(format!("`{key}` is set in the configuration file only"));
        }
        if self.flags.explicit(key).is_some() {
            return Err(format!("`{key}` is set by a command-line flag"));
        }
        Ok(())
    }
}

fn opt(v: Option<&str>) -> String {
    v.map_or_else(|| "null".to_owned(), string)
}

impl Tunables for LiveSettings {
    fn view(&self) -> String {
        let changes = saved(&self.state);
        let file = self.file().unwrap_or_default();
        let all = self.with(&changes).unwrap_or_default();
        let rows: Vec<String> = LIVE
            .iter()
            .map(|l| {
                let ui = changes.iter().rev().find(|(k, _)| k == l.key).map(|(_, v)| v.as_str());
                let choices: Vec<String> = l.choices.iter().map(|c| string(c)).collect();
                let parts: Vec<String> = parts(l.key)
                    .iter()
                    .map(|p| {
                        let c: Vec<String> = p.choices.iter().map(|c| string(c)).collect();
                        format!("{{\"label\":{},\"choices\":[{}]}}", string(p.label), c.join(","))
                    })
                    .collect();
                format!(
                    "{{\"key\":{},\"group\":{},\"label\":{},\"help\":{},\"choices\":[{}],\"multi\":{},\"parts\":[{}],\"custom\":{},\"value\":{},\"config\":{},\"default\":{},\"ui\":{},\"flag\":{}}}",
                    string(l.key),
                    string(l.group),
                    string(l.label),
                    string(l.help),
                    choices.join(","),
                    multi(l.key),
                    parts.join(","),
                    custom(l.key).map_or_else(
                        || "null".to_owned(),
                        |(label, hint)| format!("{{\"label\":{},\"hint\":{}}}", string(label), string(hint))
                    ),
                    opt(all.value(l.key)),
                    opt(file.explicit(l.key)),
                    opt(Settings::default_of(l.key)),
                    opt(ui),
                    self.flags.explicit(l.key).is_some(),
                )
            })
            .collect();
        format!("{{\"settings\":[{}]}}\n", rows.join(","))
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.changeable(key)?;
        let mut changes: Vec<(String, String)> = saved(&self.state).into_iter().filter(|(k, _)| k != key).collect();
        changes.push((key.to_owned(), value.trim().to_owned()));
        self.save(&changes)
    }

    fn reset(&self, key: &str) -> Result<(), String> {
        if live(key).is_none() {
            return Err(format!("`{key}` is set in the configuration file only"));
        }
        let changes: Vec<(String, String)> = saved(&self.state).into_iter().filter(|(k, _)| k != key).collect();
        self.save(&changes)
    }
}
