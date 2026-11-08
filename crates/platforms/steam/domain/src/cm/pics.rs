//! What the account owns and what each thing is: the license list Steam
//! sends after a logon (packages), access tokens for packages and apps, and
//! PICS product info (a package's apps and depots as binary KeyValues, an
//! app's depots and manifests as text KeyValues).
//!
//! Field numbers follow Steam's `steammessages_clientserver*.proto`.

use alloc::vec::Vec;

use super::message::CmError;
use super::proto::{Wire, Writer, fields, read_varint};

/// Message numbers.
pub mod emsg {
    pub const CLIENT_LICENSE_LIST: u32 = 780;
    pub const PICS_PRODUCT_INFO_REQUEST: u32 = 8903;
    pub const PICS_PRODUCT_INFO_RESPONSE: u32 = 8904;
    pub const PICS_ACCESS_TOKEN_REQUEST: u32 = 8905;
    pub const PICS_ACCESS_TOKEN_RESPONSE: u32 = 8906;
}

fn each<'a>(body: &'a [u8], what: &'static str, mut f: impl FnMut(u32, Wire<'a>)) -> Result<(), CmError> {
    for field in fields(body) {
        let (n, v) = field.map_err(|()| CmError::Malformed(what))?;
        f(n, v);
    }
    Ok(())
}

/// A repeated integer, one value per field or packed in one.
fn push_ints(v: Wire<'_>, out: &mut Vec<u64>) {
    match v {
        Wire::Bytes(b) => {
            let mut at = 0;
            while let Some((x, next)) = read_varint(b, at) {
                out.push(x);
                at = next;
            }
        }
        other => out.extend(other.int()),
    }
}

/// One license: a package the account owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct License {
    /// 1.
    pub package_id: u32,
    /// 17: what PICS asks for to read the package.
    pub access_token: u64,
}

/// `CMsgClientLicenseList` (2 `licenses`).
///
/// # Errors
/// Not a protobuf.
pub fn licenses(body: &[u8]) -> Result<Vec<License>, CmError> {
    let mut out = Vec::new();
    let mut bad = false;
    each(body, "license list", |n, v| {
        if n != 2 {
            return;
        }
        let mut l = License { package_id: 0, access_token: 0 };
        for f in fields(v.bytes().unwrap_or_default()) {
            match f {
                Ok((1, v)) => l.package_id = v.int().unwrap_or(0) as u32,
                Ok((17, v)) => l.access_token = v.int().unwrap_or(0),
                Ok(_) => {}
                Err(()) => bad = true,
            }
        }
        out.push(l);
    })?;
    if bad {
        return Err(CmError::Malformed("license"));
    }
    Ok(out)
}

/// `CMsgClientPICSAccessTokenRequest` (1 `packageids`, 2 `appids`).
#[must_use]
pub fn access_token_request(packages: &[u32], apps: &[u32]) -> Vec<u8> {
    let mut w = Writer::new();
    for p in packages {
        w = w.varint(1, u64::from(*p));
    }
    for a in apps {
        w = w.varint(2, u64::from(*a));
    }
    w.finish()
}

/// `CMsgClientPICSAccessTokenResponse`: tokens granted (1 packages, 3
/// apps, each `{1 id, 2 token}`) and ids denied (2 packages, 4 apps).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessTokens {
    pub packages: Vec<(u32, u64)>,
    pub apps: Vec<(u32, u64)>,
    pub denied_packages: Vec<u32>,
    pub denied_apps: Vec<u32>,
}

impl AccessTokens {
    /// # Errors
    /// Not a protobuf.
    pub fn decode(body: &[u8]) -> Result<Self, CmError> {
        let mut t = Self::default();
        let pair = |v: Wire<'_>| {
            let (mut id, mut token) = (0u32, 0u64);
            for f in fields(v.bytes().unwrap_or_default()).filter_map(Result::ok) {
                match f {
                    (1, v) => id = v.int().unwrap_or(0) as u32,
                    (2, v) => token = v.int().unwrap_or(0),
                    _ => {}
                }
            }
            (id, token)
        };
        let (mut dp, mut da) = (Vec::new(), Vec::new());
        each(body, "access tokens", |n, v| match n {
            1 => t.packages.push(pair(v)),
            2 => push_ints(v, &mut dp),
            3 => t.apps.push(pair(v)),
            4 => push_ints(v, &mut da),
            _ => {}
        })?;
        t.denied_packages = dp.into_iter().map(|x| x as u32).collect();
        t.denied_apps = da.into_iter().map(|x| x as u32).collect();
        Ok(t)
    }
}

/// `CMsgClientPICSProductInfoRequest`: packages (1) and apps (2), each
/// `{1 id, 2 access_token}`; 3 `meta_data_only` (change numbers, no data).
#[must_use]
pub fn product_info_request(packages: &[(u32, u64)], apps: &[(u32, u64)], meta_only: bool) -> Vec<u8> {
    let mut w = Writer::new();
    for (field, list) in [(1, packages), (2, apps)] {
        for (id, token) in list {
            let mut item = Writer::new().varint(1, u64::from(*id));
            if *token != 0 {
                item = item.varint(2, *token);
            }
            w = w.bytes(field, &item.finish());
        }
    }
    if meta_only {
        w = w.bool(3, true);
    }
    w.finish()
}

/// One app or package in a product info answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub id: u32,
    pub change_number: u32,
    /// Its KeyValues (text for an app, binary after a 4-byte id for a package).
    pub buffer: Vec<u8>,
    /// Steam held it back for want of an access token.
    pub missing_token: bool,
}

/// `CMsgClientPICSProductInfoResponse`; an answer may come in several
/// messages, all but the last with `pending` set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductInfo {
    /// 1: `{1 appid, 2 change_number, 3 missing_token, 5 buffer}`.
    pub apps: Vec<Product>,
    /// 2.
    pub unknown_apps: Vec<u32>,
    /// 3: `{1 packageid, 2 change_number, 3 missing_token, 5 buffer}`.
    pub packages: Vec<Product>,
    /// 4.
    pub unknown_packages: Vec<u32>,
    /// 6 `response_pending`.
    pub pending: bool,
}

fn product(v: Wire<'_>) -> Product {
    let mut p = Product { id: 0, change_number: 0, buffer: Vec::new(), missing_token: false };
    for f in fields(v.bytes().unwrap_or_default()).filter_map(Result::ok) {
        match f {
            (1, v) => p.id = v.int().unwrap_or(0) as u32,
            (2, v) => p.change_number = v.int().unwrap_or(0) as u32,
            (3, v) => p.missing_token = v.int() == Some(1),
            (5, v) => p.buffer = v.bytes().unwrap_or_default().to_vec(),
            _ => {}
        }
    }
    p
}

impl ProductInfo {
    /// # Errors
    /// Not a protobuf.
    pub fn decode(body: &[u8]) -> Result<Self, CmError> {
        let mut p = Self::default();
        let (mut ua, mut up) = (Vec::new(), Vec::new());
        each(body, "product info", |n, v| match n {
            1 => p.apps.push(product(v)),
            2 => push_ints(v, &mut ua),
            3 => p.packages.push(product(v)),
            4 => push_ints(v, &mut up),
            6 => p.pending = v.int() == Some(1),
            _ => {}
        })?;
        p.unknown_apps = ua.into_iter().map(|x| x as u32).collect();
        p.unknown_packages = up.into_iter().map(|x| x as u32).collect();
        Ok(p)
    }
}
