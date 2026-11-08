//! Small whole-file GETs from origin servers through the [`Upstream`] port,
//! for lanstash's own needs (art, Battle.net's catalog and versions), as
//! opposed to the client requests [`crate::fetch`] stores.

use lanstash_ports::upstream::{Source, Upstream, UpstreamRequest};

/// Why a GET gave no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginError {
    /// 404 or 410: the server has no such file.
    Missing,
    /// Not reachable, another status, or over the size limit.
    Failed(String),
}

/// GETs `src` and reads its whole body, at most `max` bytes.
///
/// # Errors
/// [`OriginError`] as above.
pub fn get(upstream: &dyn Upstream, src: &Source, max: usize) -> Result<Vec<u8>, OriginError> {
    get_range(upstream, src, None, max)
}

/// GETs bytes `start..end` of `src` (all of it for `None`), at most `max` bytes. A range must
/// come back as one (206): a server that ignores it would send the whole file.
///
/// # Errors
/// [`OriginError`] as above.
pub fn get_range(
    upstream: &dyn Upstream,
    src: &Source,
    range: Option<(u64, u64)>,
    max: usize,
) -> Result<Vec<u8>, OriginError> {
    let asked = range.map(|(a, b)| format!("bytes={a}-{}", b.saturating_sub(1)));
    let headers: Vec<(&str, &str)> = asked.as_deref().map(|r| ("Range", r)).into_iter().collect();
    let req =
        UpstreamRequest { method: "GET", host: &src.host, port: src.port, target: &src.target, headers: &headers };
    let mut resp = upstream.fetch(&req).map_err(|e| OriginError::Failed(e.message))?;
    let head = resp.head();
    match head.status {
        200 if range.is_none() => {}
        206 if range.is_some() => {}
        404 | 410 => return Err(OriginError::Missing),
        s => return Err(OriginError::Failed(format!("{}{}: status {s}", src.host, src.target))),
    }
    let too_large = || OriginError::Failed(format!("{}{}: over {max} bytes", src.host, src.target));
    if head.content_length().is_some_and(|n| n > max as u64) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = resp.read(&mut buf).map_err(|e| OriginError::Failed(e.message))?;
        if n == 0 {
            return Ok(body);
        }
        if body.len() + n > max {
            return Err(too_large());
        }
        body.extend_from_slice(&buf[..n]);
    }
}
