//! How a rewrite is written in Pi-hole, and read back (as of
//! Pi-hole 2026.10.0, FTL's v6 API):
//!   - to an address: a regex deny rule with a reply, `^name$;reply=ADDRESS`
//!     for `name`, `[.]name$;reply=ADDRESS` for `*.name` (its subdomains only),
//!     dots as `[.]` (the API refuses a JSON body with a backslash in it). It
//!     answers A with the address and AAAA with `::`, so IPv6 clients cannot go
//!     around the cache; a local DNS record (`dns.hosts`) answers A only and
//!     lets AAAA through to the internet, so lanstash writes none;
//!   - to a name: a local CNAME record, `name,target` (`dns.cnameRecords`),
//!     which Pi-hole follows only to a target with a local DNS record and no
//!     deny rule (it inspects CNAME targets; a reply rule is a deny rule): the
//!     target gets a record, to the address it has in Pi-hole, and loses its
//!     reply rule. Pi-hole has CNAMEs for whole names only: a `*.name` pointed
//!     at a name is a reply rule to that name's address;
//!   - read back too: local DNS records (`ADDRESS name ...`), and rules a
//!     person wrote with `\.` for the dots.
//!
//! A rule that is not one of these shapes is not a rewrite and is left alone.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use lanstash_ports::dns::Rewrite;

/// Whether `a` reads as an IP address.
#[must_use]
pub fn is_address(a: &str) -> bool {
    let v4 = a.split('.').count() == 4 && a.split('.').all(|p| p.parse::<u8>().is_ok());
    v4 || (a.contains(':') && a.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.'))
}

/// Whether `name` is a host name a rule can be written for (letters, digits, `-`, dots).
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

/// The regex rule of a rewrite of `pattern` (`name` or `*.name`) to `address`.
///
/// # Errors
/// A pattern that is not a host name (or `*.` and one), or an answer that is not an address.
pub fn regex(pattern: &str, address: &str) -> Result<String, String> {
    if !is_address(address) {
        return Err(format!("{address} is not an address"));
    }
    let (start, name) = match pattern.strip_prefix("*.") {
        Some(base) => ("[.]", base),
        None => ("^", pattern),
    };
    if !plain(name) {
        return Err(format!("{pattern} is not a host name or *. and one"));
    }
    Ok(format!("{start}{}$;reply={address}", name.to_ascii_lowercase().replace('.', "[.]")))
}

/// The rewrite a regex rule is, if it is one: `^name$` or `[.]name$` (or `\.name$`), with a reply address.
#[must_use]
pub fn from_regex(rule: &str) -> Option<Rewrite> {
    let (body, options) = rule.split_once(';')?;
    let address = options.split(';').find_map(|o| o.strip_prefix("reply=")).filter(|a| is_address(a))?;
    let body = body.strip_suffix('$')?;
    let (wild, name) = if let Some(n) = body.strip_prefix('^') {
        (false, n)
    } else if let Some(n) = body.strip_prefix("[.]").or_else(|| body.strip_prefix("\\.")) {
        (true, n)
    } else {
        return None;
    };
    let name = name.replace("[.]", ".").replace("\\.", ".");
    if !plain(&name) {
        return None;
    }
    let pattern = if wild { format!("*.{name}") } else { name };
    Some(Rewrite { pattern, answer: address.to_string() })
}

/// The rewrites a local DNS record (`ADDRESS name ...`) is.
#[must_use]
pub fn from_host(line: &str) -> Vec<Rewrite> {
    let mut words = line.split_whitespace();
    let Some(address) = words.next().filter(|a| is_address(a)) else { return Vec::new() };
    words.filter(|n| plain(n)).map(|n| Rewrite { pattern: n.to_string(), answer: address.to_string() }).collect()
}

/// The rewrite a local CNAME record (`name,target[,ttl]`) is.
#[must_use]
pub fn from_cname(entry: &str) -> Option<Rewrite> {
    let mut parts = entry.split(',').map(str::trim);
    let (name, target) = (parts.next()?, parts.next()?);
    (plain(name) && plain(target)).then(|| Rewrite { pattern: name.to_string(), answer: target.to_string() })
}

/// `text` for a path segment: everything but letters, digits and `-._~` percent-encoded.
#[must_use]
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
