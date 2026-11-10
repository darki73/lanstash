//! The generic documents of the admin API: status snapshots with their
//! rates, prefill status, the deletion of a game, the clients downloading
//! now, and error bodies.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use lanstash_app::client_names::ClientName;
use lanstash_app::delete_game::DeleteStatus;
use lanstash_app::serve::clients::Downloading;
use lanstash_app::status::{CounterGroup, Snapshot};
use lanstash_domain::json::string;
use lanstash_ports::prefill::PrefillStatus;

/// Byte counters at the previous snapshot of one viewer, for rates.
pub type Sample = (u64, BTreeMap<String, u64>);

/// Rates in MB/s of the byte counters since `prev`, and the new sample.
#[must_use]
pub fn rates(prev: Option<&Sample>, now_ms: u64, groups: &[CounterGroup]) -> (BTreeMap<String, f64>, Sample) {
    let mut cur = BTreeMap::new();
    for g in groups {
        for (n, v) in &g.values {
            if n.contains("bytes") {
                cur.insert(format!("{}.{n}", g.name), *v);
            }
        }
    }
    let mut out = BTreeMap::new();
    if let Some((t, before)) = prev {
        let secs = now_ms.saturating_sub(*t) as f64 / 1000.0;
        if secs >= 0.2 {
            for (k, v) in &cur {
                let b = before.get(k).copied().unwrap_or(0);
                out.insert(k.clone(), v.saturating_sub(b) as f64 / 1e6 / secs);
            }
        }
    }
    (out, (now_ms, cur))
}

/// A snapshot as JSON (`web/src/contracts/status` reads it).
#[must_use]
pub fn render_json(s: &Snapshot, rates: &BTreeMap<String, f64>) -> String {
    let mut j = String::new();
    let _ = write!(j, "{{\"now_ms\":{},\"started_ms\":{},\"counters\":{{", s.now_ms, s.started_ms);
    for (i, g) in s.counters.iter().enumerate() {
        let vals: Vec<String> = g.values.iter().map(|(n, v)| format!("{}:{v}", string(n))).collect();
        let _ = write!(j, "{}{}:{{{}}}", if i > 0 { "," } else { "" }, string(g.name), vals.join(","));
    }
    let r: Vec<String> = rates.iter().map(|(k, v)| format!("{}:{v:.3}", string(k))).collect();
    let _ = write!(j, "}},\"rates_mb_s\":{{{}}},\"downloads\":[", r.join(","));
    for (i, d) in s.downloads.iter().enumerate() {
        // An unknown end (`u64::MAX`) is not exact in JavaScript: null.
        let span = d.span.map_or_else(
            || "null".to_owned(),
            |sp| if sp.end == u64::MAX { format!("[{},null]", sp.start) } else { format!("[{},{}]", sp.start, sp.end) },
        );
        let total = d.total.map_or_else(|| "null".to_owned(), |t| t.to_string());
        let _ = write!(
            j,
            "{}{{\"key\":{},\"span\":{span},\"received\":{},\"total\":{total}}}",
            if i > 0 { "," } else { "" },
            string(&d.key),
            d.received
        );
    }
    let _ = write!(j, "],\"usage_ms\":{},\"usage\":[", s.usage_ms);
    for (i, row) in s.usage.iter().enumerate() {
        let name = row.name.as_deref().map_or_else(|| "null".to_owned(), string);
        let group = row.group.as_deref().map_or_else(|| "null".to_owned(), string);
        let _ = write!(
            j,
            "{}{{\"service\":{},\"game\":{},\"name\":{name},\"group\":{group},\"objects\":{},\"bytes\":{},\"last_used_ms\":{}}}",
            if i > 0 { "," } else { "" },
            string(&row.game.service),
            string(&row.game.game),
            row.usage.objects,
            row.usage.bytes,
            row.usage.last_used_ms
        );
    }
    let prefill = s.prefill.as_ref().map_or_else(|| "null".to_owned(), |(id, p)| prefill_on(id, p));
    let deleting = s.deleting.as_ref().map_or_else(|| "null".to_owned(), delete_json);
    let prefills: Vec<String> =
        s.prefills.iter().map(|(id, p)| format!("{}:{}", string(id), prefill_json(p))).collect();
    let changed: Vec<String> = s.builds_changed.iter().map(|(id, ms)| format!("{}:{ms}", string(id))).collect();
    let clients: Vec<String> = s.clients.iter().map(|(d, n)| downloading_json(d, n)).collect();
    let _ = write!(
        j,
        "],\"prefill\":{prefill},\"prefills\":{{{}}},\"deleting\":{deleting},\"builds_changed\":{{{}}},\"clients\":[{}]}}",
        prefills.join(","),
        changed.join(","),
        clients.join(",")
    );
    j
}

/// A client's name as JSON fields: `"address","label","host"` (each name null when none).
#[must_use]
pub fn client_name_fields(address: &str, n: &ClientName) -> String {
    let opt = |v: Option<&str>| v.map_or_else(|| "null".to_owned(), string);
    format!("\"address\":{},\"label\":{},\"host\":{}", string(address), opt(n.label.as_deref()), opt(n.host.as_deref()))
}

/// A client downloading a game now, as JSON.
#[must_use]
pub fn downloading_json(d: &Downloading, n: &ClientName) -> String {
    format!(
        "{{{},\"service\":{},\"game\":{},\"per_sec\":{},\"hit_bytes\":{},\"miss_bytes\":{},\"total_bytes\":{},\"session_bytes\":{},\"since_ms\":{},\"last_ms\":{}}}",
        client_name_fields(&d.client, n),
        string(&d.service),
        string(&d.game),
        d.per_sec,
        d.hit_bytes,
        d.miss_bytes,
        d.total_bytes,
        d.session_bytes,
        d.since_ms,
        d.last_ms
    )
}

/// One prefill's status as JSON, with its platform first (so a hidden game's job can be masked).
#[must_use]
pub fn prefill_on(platform: &str, p: &PrefillStatus) -> String {
    let job = prefill_json(p);
    format!("{{\"platform\":{},{}", string(platform), job.strip_prefix('{').unwrap_or(&job))
}

/// One prefill's status as JSON.
#[must_use]
pub fn prefill_json(p: &PrefillStatus) -> String {
    let opt = |v: Option<&str>| v.map_or_else(|| "null".to_owned(), string);
    format!(
        "{{\"game\":{},\"version\":{},\"prerelease\":{},\"install\":{},\"from\":{},\"state\":{},\"files_total\":{},\"files_done\":{},\"files_skipped\":{},\"bytes_skipped\":{},\"files_missing\":{},\"files_local\":{},\"bytes_local\":{},\"files_rejected\":{},\"bytes_total\":{},\"bytes\":{},\"current\":{},\"error\":{},\"started_ms\":{},\"finished_ms\":{},\"planning\":{},\"queued\":[{}]}}",
        string(&p.game),
        opt(p.version.as_deref()),
        p.prerelease,
        opt(p.install.as_deref()),
        string(p.from.as_str()),
        string(p.state.as_str()),
        p.files_total,
        p.files_done,
        p.files_skipped,
        p.bytes_skipped,
        p.files_missing,
        p.files_local,
        p.bytes_local,
        p.files_rejected,
        p.bytes_total,
        p.bytes,
        opt(p.current.as_deref()),
        opt(p.error.as_deref()),
        p.started_ms,
        p.finished_ms.map_or_else(|| "null".to_owned(), |t| t.to_string()),
        p.planning.map_or_else(
            || "null".to_owned(),
            |g| format!("{{\"done\":{},\"total\":{},\"step\":{}}}", g.done, g.total, opt(g.step.map(|s| s.as_str())))
        ),
        p.queued.iter().map(|g| string(g)).collect::<Vec<_>>().join(","),
    )
}

/// The last deletion of a game as JSON.
#[must_use]
pub fn delete_json(d: &DeleteStatus) -> String {
    format!(
        "{{\"platform\":{},\"game\":{},\"state\":{},\"items_total\":{},\"items_done\":{},\"bytes_total\":{},\"bytes_done\":{},\"failed\":{},\"started_ms\":{},\"finished_ms\":{},\"partial\":{},\"bytes_kept\":{}}}",
        string(&d.platform),
        string(&d.game),
        string(d.state.as_str()),
        d.items_total,
        d.items_done,
        d.bytes_total,
        d.bytes_done,
        d.failed.len(),
        d.started_ms,
        d.finished_ms.map_or_else(|| "null".to_owned(), |t| t.to_string()),
        d.partial,
        d.bytes_kept,
    )
}

/// An API error body: what went wrong, in words, and its kind, a word a
/// program can test (`not-found`, `method`, `forbidden`, ...).
#[must_use]
pub fn error_json(kind: &str, message: &str) -> String {
    format!("{{\"error\":{},\"kind\":{}}}\n", string(message), string(kind))
}
