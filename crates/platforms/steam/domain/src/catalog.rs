//! Steam's catalog view (`GET /api/v1/catalog/steam`), in the shape of
//! Battle.net's so the web UI offers both alike: the accounts' games (apps
//! of type Game with something to install for the target), each with its
//! app id (code and game), name, public build, download size (when the app
//! info gives one), how much of it lancache's old cache holds (an
//! estimate, once probed), which accounts own it (`owners`, when known) and
//! the operating systems it installs for (`systems`, when more than one).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::json::string;

use crate::apps::{App, Target};

/// Whether an app is offered: a game with depots for `t`.
#[must_use]
pub fn offered(app: &App, t: &Target) -> bool {
    app.kind.as_deref().is_some_and(|k| k.eq_ignore_ascii_case("game")) && !app.install(t).is_empty()
}

/// What lancache's old cache holds of an app's current build (have, want), when known.
pub type ShareOf<'a> = &'a dyn Fn(&App) -> Option<(u64, u64)>;

/// Bytes an install of an app downloads (its own depots, owned DLCs', shared ones).
pub type SizeOf<'a> = &'a dyn Fn(&App) -> u64;

/// The accounts that own an app, in the order they signed in.
pub type OwnersOf<'a> = &'a dyn Fn(&App) -> Vec<String>;

/// The view of `apps` (the library's), by name, with each one's download size,
/// what lancache holds of it, and its owners (where the library knows them).
#[must_use]
pub fn render(apps: &[App], t: &Target, size: SizeOf<'_>, share: ShareOf<'_>, owners: OwnersOf<'_>) -> String {
    let mut games: Vec<&App> = apps.iter().filter(|a| offered(a, t)).collect();
    games.sort_by_key(|a| a.title().to_lowercase());
    let products: Vec<String> = games
        .iter()
        .map(|a| {
            let version = a.build.map_or_else(|| String::from("null"), |b| format!("\"{b}\""));
            // Older app info gives no sizes: the size is left out rather than shown as 0.
            let download = match size(a) {
                0 => String::new(),
                n => format!(",\"download\":{n}"),
            };
            let lancache = share(a).map_or_else(String::new, |(have, want)| {
                format!(",\"lancache\":{{\"have\":{have},\"want\":{want}}}")
            });
            let owners = match owners(a) {
                o if o.is_empty() => String::new(),
                o => format!(",\"owners\":[{}]", o.iter().map(|n| string(n)).collect::<Vec<_>>().join(",")),
            };
            // Another system to prefill for: only when it has more than the one set up.
            let systems = match a.systems(t) {
                s if s.len() < 2 => String::new(),
                s => format!(",\"systems\":[{}]", s.iter().map(|o| string(o)).collect::<Vec<_>>().join(",")),
            };
            format!(
                "{{\"code\":\"{id}\",\"name\":{},\"game\":\"{id}\",\"version\":{version},\"known\":true{download}{lancache}{owners}{systems}}}",
                string(&a.title()),
                id = a.id,
            )
        })
        .collect();
    format!("{{\"products\":[{}]}}\n", products.join(","))
}

/// The view while there is no library: no games, and why (it is being read, or reading it failed).
#[must_use]
pub fn render_none(why: &str) -> String {
    format!("{{\"products\":[],\"error\":{}}}\n", string(why))
}
