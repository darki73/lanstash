//! Names the server already sends to lanstash without a rewrite of their own:
//! a rule of its own, a filter list it follows, or a rewrite of a wider pattern
//! ([`DnsServer::check`], for servers that can say). They are not missing:
//! Apply adds no rewrite for them, and they are shown with what answers them.

use lanstash_ports::dns::{DnsServer, Rewrite};

use super::{Plan, resolve};

/// The name a pattern is asked about: itself, or for `*.name` a name under it.
#[must_use]
pub fn sample(pattern: &str) -> String {
    pattern.strip_prefix("*.").map_or_else(|| pattern.to_owned(), |base| format!("lanstash.{base}"))
}

/// What answers a name for lanstash when it is not a rule (a rewrite of another pattern).
pub const BY_REWRITE: &str = "another rewrite";

/// Moves the missing patterns `server` answers with lanstash's address anyway to `plan.covered`.
/// One the server cannot say about (or fails to) stays missing.
pub fn split(server: &dyn DnsServer, current: &[Rewrite], plan: &mut Plan) {
    let mut missing = Vec::new();
    for p in std::mem::take(&mut plan.missing) {
        let answered = server.check(&sample(&p)).and_then(Result::ok);
        let ours = answered.filter(|a| a.to.as_deref().is_some_and(|t| resolve(current, t) == plan.answer));
        match ours {
            Some(a) => plan.covered.push((p, a.rule.unwrap_or_else(|| BY_REWRITE.to_owned()))),
            None => missing.push(p),
        }
    }
    plan.missing = missing;
}
