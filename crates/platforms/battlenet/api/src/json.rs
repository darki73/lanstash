//! Battle.net's views as JSON, in the shapes `web/src/contracts` reads.

use std::sync::Arc;

use lanstash_battlenet_app::builds::{BlizzardBuilds, BuildCheck, BuildState, Flavour};
use lanstash_battlenet_app::nginx::{ProbeReport, Share};
use lanstash_battlenet_app::products::CatalogProduct;
use lanstash_battlenet_app::prune::Prunable;
use lanstash_battlenet_domain::nginx_keys::Tally;
use lanstash_battlenet_domain::plan::PlanDetail;
use lanstash_domain::json::string;

fn opt(v: Option<&str>) -> String {
    v.map_or_else(|| "null".to_owned(), string)
}

fn num(v: Option<u64>) -> String {
    v.map_or_else(|| "null".to_owned(), |n| n.to_string())
}

/// A build: `version`, `build_config`, `have` and `want` (bytes of its
/// install stashed and in all; `null` until planned), `complete`, `plan_error`.
#[must_use]
pub fn build(b: &BuildState) -> String {
    format!(
        "{{\"version\":{},\"build_config\":{},\"have\":{},\"want\":{},\"complete\":{},\"plan_error\":{}}}",
        string(&b.version.name),
        string(&b.version.build_config),
        num(b.coverage.map(|c| c.have)),
        num(b.coverage.map(|c| c.want)),
        b.complete(),
        opt(b.plan_error.as_deref()),
    )
}

/// `{"region": "...", "lancache": bool, "games": [{"game", "product", "version",
/// "build_config", "current", "have", "want", "plan_error", "prerelease": build | null,
/// "error"}]}`: the current build's fields inline, the pre-release build as
/// [`build`]; `lancache`: whether a prefill can take pieces from lancache's cache;
/// `lancache_share`: `{"have", "want"}` bytes of the current build lancache's cache holds
/// (`share`, given the game and build config; asked only for a build not stashed whole, so
/// complete games cost the NAS no probe), `null` until known or when stashed whole.
#[must_use]
pub fn builds(b: &BlizzardBuilds, lancache: bool, share: &dyn Fn(&str, &str) -> Option<Share>) -> String {
    builds_of(&b.check_all(), b.region(), lancache, share)
}

/// As [`builds`], of games already checked in `region`.
#[must_use]
pub fn builds_of(
    checks: &[BuildCheck],
    region: &str,
    lancache: bool,
    share: &dyn Fn(&str, &str) -> Option<Share>,
) -> String {
    let games: Vec<String> = checks
        .iter()
        .map(|c| {
            let current = c.build.as_ref().map_or_else(
                || "\"version\":null,\"build_config\":null,\"have\":null,\"want\":null,\"complete\":false,\"plan_error\":null".to_owned(),
                |b| {
                    let j = build(b);
                    j[1..j.len() - 1].to_owned() // its fields, inline
                },
            );
            format!(
                "{{\"game\":{},\"product\":{},{current},\"current\":{},\"prerelease\":{},\"error\":{},\"flavours\":{},\"lancache_share\":{}}}",
                string(&c.game),
                opt(c.product.as_deref()),
                c.current(),
                c.prerelease.as_ref().map_or_else(|| "null".to_owned(), build),
                opt(c.error.as_deref()),
                flavours(&c.flavours, &|_, s| build(s)),
                c.build.as_ref().filter(|s| !s.complete()).and_then(|s| share(&c.game, &s.version.build_config)).map_or_else(
                    || "null".to_owned(),
                    |s| format!("{{\"have\":{},\"want\":{}}}", s.have, s.want),
                ),
            )
        })
        .collect();
    format!("{{\"region\":{},\"lancache\":{lancache},\"games\":[{}]}}\n", string(region), games.join(","))
}

/// `{"products": [{"code", "name", "family", "game", "version", "known"}]}`: one entry per
/// install, `family` the name of the catalog product it installs.
#[must_use]
pub fn catalog(list: &[CatalogProduct]) -> String {
    let products: Vec<String> = list
        .iter()
        .map(|p| {
            format!(
                "{{\"code\":{},\"name\":{},\"family\":{},\"game\":{},\"version\":{},\"known\":{}}}",
                string(&p.code),
                opt(p.name.as_deref()),
                opt(p.family.as_deref()),
                opt(p.game.as_deref()),
                opt(p.version.as_deref()),
                p.known,
            )
        })
        .collect();
    format!("{{\"products\":[{}]}}\n", products.join(","))
}

/// A plan's figures (`null` until the plan is made).
#[must_use]
pub fn detail(d: Option<PlanDetail>) -> String {
    d.map_or_else(
        || "null".to_owned(),
        |d| {
            format!(
                "{{\"files_have\":{},\"files_want\":{},\"archived_have\":{},\"archived_want\":{},\"file_count\":{},\"part_count\":{},\"archive_count\":{},\"requests\":{},\"absent\":{}}}",
                d.files.have,
                d.files.want,
                d.archived.have,
                d.archived.want,
                d.file_count,
                d.part_count,
                d.archive_count,
                d.requests,
                d.absent,
            )
        },
    )
}

/// `{"game", "product", "region", "build": build + "detail" | null, "prerelease": ... | null,
/// "error", "lancache", "installs": {"platforms", "architectures", "locales", "own", "held":
/// {<platform>: {"have", "want"} | null}} | null}` (`lancache`: whether a prefill can take it from
/// lancache's cache; `installs`: what the current build has installs for, once its plan is made, so
/// a prefill may name another platform's or locale's; `own`: the platform set up, if one of them;
/// `held`: how much of each platform's install the stash holds, null while its plan is being made).
#[must_use]
pub fn game(b: &BlizzardBuilds, game: &str, lancache: bool) -> String {
    let c = b.check(game);
    let with_detail = |product: &str, s: &BuildState| {
        let figures = b
            .plans()
            .and_then(|plans| plans.ready(product, &s.version).and_then(Result::ok).map(|plan| plans.detail(&plan)));
        let j = build(s);
        format!("{},\"detail\":{}}}", &j[..j.len() - 1], detail(figures))
    };
    let main = |s: &BuildState| c.product.as_deref().map_or_else(|| build(s), |p| with_detail(p, s));
    // What the current build has installs for, once its plan is made.
    let installs = match (&c.product, &c.build, b.plans()) {
        (Some(p), Some(s), Some(plans)) => plans.ready(p, &s.version).and_then(Result::ok).map(|plan| {
            let list = |v: &[String]| v.iter().map(|t| string(t)).collect::<Vec<_>>().join(",");
            let i = &plan.installs;
            // What the stash holds of each platform's install (its own plan, made in the background:
            // null until then); the platform set up is the build's own plan.
            let held: Vec<String> = i
                .platforms
                .iter()
                .map(|platform| {
                    let other = || plans.ready_for(p, &s.version, &[platform.as_str()]).and_then(Result::ok);
                    let made = if plans.set_up(platform) { Some(Arc::clone(&plan)) } else { other() };
                    let cover = made.map_or_else(
                        || "null".to_owned(),
                        |m| {
                            let c = plans.coverage(&m);
                            format!("{{\"have\":{},\"want\":{}}}", c.have, c.want)
                        },
                    );
                    format!("{}:{cover}", string(platform))
                })
                .collect();
            let own = i.platforms.iter().find(|x| plans.set_up(x)).map_or_else(|| "null".to_owned(), |x| string(x));
            format!(
                "{{\"platforms\":[{}],\"architectures\":[{}],\"locales\":[{}],\"own\":{own},\"held\":{{{}}}}}",
                list(&i.platforms),
                list(&i.architectures),
                list(&i.locales),
                held.join(",")
            )
        }),
        _ => None,
    };
    format!(
        "{{\"game\":{},\"product\":{},\"region\":{},\"build\":{},\"prerelease\":{},\"error\":{},\"lancache\":{},\"installs\":{},\"flavours\":{}}}\n",
        string(game),
        opt(c.product.as_deref()),
        string(b.region()),
        c.build.as_ref().map_or_else(|| "null".to_owned(), main),
        c.prerelease.as_ref().map_or_else(|| "null".to_owned(), main),
        opt(c.error.as_deref()),
        lancache,
        installs.unwrap_or_else(|| "null".to_owned()),
        flavours(&c.flavours, &with_detail),
    )
}

/// `[{"product", "name", "build", "prerelease"}]`: the other products of a
/// directory, each build drawn by `draw` (given the product code).
fn flavours(list: &[Flavour], draw: &dyn Fn(&str, &BuildState) -> String) -> String {
    let one = |f: &Flavour| {
        let b = |s: &Option<BuildState>| s.as_ref().map_or_else(|| "null".to_owned(), |s| draw(&f.product, s));
        format!(
            "{{\"product\":{},\"name\":{},\"build\":{},\"prerelease\":{}}}",
            string(&f.product),
            opt(f.name.as_deref()),
            b(&f.build),
            b(&f.prerelease)
        )
    };
    format!("[{}]", list.iter().map(one).collect::<Vec<_>>().join(","))
}

/// What an nginx cache holds of a game's build.
#[must_use]
pub fn probe(r: &ProbeReport) -> String {
    let tally = |t: &Tally| {
        format!(
            "{{\"wanted\":{},\"found\":{},\"bytes\":{},\"bytes_found\":{}}}",
            t.wanted, t.found, t.bytes, t.bytes_found
        )
    };
    let missing: Vec<String> =
        r.missing.iter().map(|(k, f)| format!("{{\"key\":{},\"file\":{}}}", string(k), string(f))).collect();
    format!(
        "{{\"game\":{},\"product\":{},\"version\":{},\"whole\":{},\"archived\":{},\"slices\":{},\"slices_found\":{},\"missing\":[{}]}}\n",
        string(&r.game),
        string(&r.product),
        string(&r.version),
        tally(&r.whole),
        tally(&r.archived),
        r.slices,
        r.slices_found,
        missing.join(","),
    )
}

/// `{"game", "files", "bytes", "recent_bytes"}`: what a prune of `game` would delete (object
/// files no current build needs, unused for the grace period), and what it keeps as used lately.
#[must_use]
pub fn prune(game: &str, r: &Prunable) -> String {
    format!(
        "{{\"game\":{},\"files\":{},\"bytes\":{},\"recent_bytes\":{}}}\n",
        string(game),
        r.files.len(),
        r.bytes,
        r.recent_bytes
    )
}
