//! Which requests the admin API answers at all. It has no sign-in: anyone on the network may
//! use it. What it refuses is a web page elsewhere using it through a visitor's browser:
//!   - a state-changing request (not `GET` or `HEAD`) the browser says comes from another
//!     site (`Origin` not this host, or `Sec-Fetch-Site` not `same-origin`); tools that send
//!     neither (the command line, curl) are not browsers and pass;
//!   - any request whose `Host` is a name someone outside could own (DNS rebinding: a page
//!     whose own name is made to point at lanstash). Addresses, `localhost`, names of one
//!     label and names under a suffix that is never delegated (`.local`, `.lan`,
//!     `.home.arpa`, `.internal`, `.localhost`) are the network's own.
//!
//! The same rule says where the plain port, with HTTPS on, may send a browser ([`https_redirect`]).

use lanstash_domain::http::RequestHead;

/// Suffixes no public name can have.
const LOCAL_SUFFIXES: &[&str] = &[".local", ".lan", ".home.arpa", ".internal", ".localhost"];

/// Why `req` is refused, if it is.
#[must_use]
pub fn refused(req: &RequestHead) -> Option<&'static str> {
    let host = req.header("host");
    if host.is_some_and(|h| !local_host(h)) {
        return Some("this name is not the network's own: use lanstash's address or a local name");
    }
    if matches!(req.method.as_str(), "GET" | "HEAD") {
        return None;
    }
    if req.header("sec-fetch-site").is_some_and(|s| s != "same-origin" && s != "none") {
        return Some("a request from another site");
    }
    let same = |origin: &str| host.is_some_and(|h| authority(origin).eq_ignore_ascii_case(h.trim()));
    match req.header("origin") {
        Some(origin) if !same(origin) => Some("a request from another site"),
        _ => None,
    }
}

/// Where the plain admin port sends `req` once HTTPS is on (`https_port`): the same path, on the
/// same name, over HTTPS. None for a request without a name of the network's own (the redirect is
/// never built from a name someone outside could own, nor from anything but a name or an address),
/// or whose target is not a path.
#[must_use]
pub fn https_redirect(req: &RequestHead, https_port: u16) -> Option<String> {
    let host = req.header("host")?.trim();
    if !local_host(host) || !req.target.starts_with('/') || req.target.starts_with("//") {
        return None;
    }
    // `name:port`, or `[v6]:port`: the name without the plain port.
    let name = match host.rsplit_once(':') {
        Some((n, p)) if p.bytes().all(|b| b.is_ascii_digit()) && !n.ends_with(':') => n,
        _ => host,
    };
    let plain = name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-.[]:".contains(&b));
    (plain && !name.is_empty()).then(|| format!("https://{name}:{https_port}{}", req.target))
}

/// `scheme://host[:port]` as `host[:port]` (`null` and the like stay as they are).
fn authority(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, a)| a).trim_end_matches('/')
}

/// Whether `host` (`name[:port]`, `[v6]:port`) is an address or a name of the network's own.
fn local_host(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let name = if let Some(rest) = h.strip_prefix('[') {
        return rest.split_once(']').is_some_and(|(v6, _)| v6.parse::<core::net::Ipv6Addr>().is_ok());
    } else {
        h.rsplit_once(':').map_or(h.as_str(), |(n, p)| if p.bytes().all(|b| b.is_ascii_digit()) { n } else { "" })
    };
    let name = name.trim_end_matches('.');
    !name.is_empty()
        && (name.parse::<core::net::IpAddr>().is_ok()
            || name == "localhost"
            || !name.contains('.')
            || LOCAL_SUFFIXES.iter().any(|s| name.ends_with(s)))
}
