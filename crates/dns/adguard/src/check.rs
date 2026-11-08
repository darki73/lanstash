//! How AdGuard Home answers a name now: `GET /control/filtering/check_host?name=`
//! (as of AdGuard Home v0.107.62). Its `reason` says what answers:
//!   - `Rewrite`: a DNS rewrite; `ip_addrs` holds the address it leads to and
//!     `cname` the name it went through, if any;
//!   - `RewriteRule`: a `$dnsrewrite` rule; `ip_addrs` is null, `cname` holds a
//!     name answer, and an address is only in the rule (`$dnsrewrite=10.0.0.1`
//!     or `$dnsrewrite=NOERROR;A;10.0.0.1`);
//!   - `FilteredBlackList`: a blocking rule, among them hosts-file rules
//!     (`10.0.0.1 name`), whose address is the answer (`0.0.0.0` or `::`: blocked);
//!   - `NotFilteredNotFound` and the rest: nothing of its own, upstream answers.
//!
//! Rules apply a moment after they are saved (they are reloaded in the background).

use alloc::string::{String, ToString};

use lanstash_domain::json::Value;
use lanstash_ports::dns::Answered;

/// Whether `a` is an address that answers nothing (`0.0.0.0`, `::`): a blocked name.
#[must_use]
pub fn unspecified(a: &str) -> bool {
    a == "0.0.0.0" || a == "::"
}

/// What a `$dnsrewrite` rule answers: its value, or the value of its long form (`NOERROR;A;value`).
fn rewritten(rule: &str) -> Option<String> {
    let (_, v) = rule.split_once("dnsrewrite=")?;
    let v = v.split(',').next().unwrap_or_default();
    let v = v.split(';').nth(2).unwrap_or(v).trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// What a hosts-file rule (`address name ...`) answers; none for any other rule, or a blocking address.
fn hosts(rule: &str) -> Option<String> {
    let first = rule.split_whitespace().next()?;
    let address = first.split('.').count() == 4 && first.split('.').all(|p| p.parse::<u8>().is_ok());
    let address = address || (first.contains(':') && first.bytes().all(|b| b.is_ascii_hexdigit() || b == b':'));
    (address && !unspecified(first) && rule.split_whitespace().nth(1).is_some()).then(|| first.to_string())
}

/// `check_host`'s answer as [`Answered`].
#[must_use]
pub fn answered(v: &Value) -> Answered {
    let text = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(ToString::to_string);
    let reason = text("reason").unwrap_or_default();
    let rule = text("rule").or_else(|| {
        let rules = v.get("rules").map(Value::as_array).unwrap_or_default();
        rules.first().and_then(|r| r.get("text")?.as_str().map(ToString::to_string))
    });
    let address = v.get("ip_addrs").map(Value::as_array).unwrap_or_default().first().and_then(|a| a.as_str());
    let address = address.filter(|a| !unspecified(a)).map(ToString::to_string);
    let cname = text("cname");
    match reason.as_str() {
        r if r.starts_with("Rewrite") && r != "RewriteRule" => Answered { to: address.or(cname), rule: None },
        "RewriteRule" => {
            let to = address.or(cname).or_else(|| rule.as_deref().and_then(rewritten));
            Answered { to, rule }
        }
        "FilteredBlackList" => Answered { to: rule.as_deref().and_then(hosts), rule },
        _ => Answered { to: None, rule: None },
    }
}
