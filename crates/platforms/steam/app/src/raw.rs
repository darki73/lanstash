//! Steam's answers a library is made of ([`Raw`]: the depots an account's
//! licenses grant, and each app's info as text), and how they are kept between
//! runs as a blob: one account's (`LSL1`, [`encode_raw`]) or several accounts'
//! (`LSL2`, [`encode_raws`]; an `LSL1` one read as the first account's).

/// Steam's answers a library is made of: the depots the licenses grant, and each app's info as text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Raw {
    pub owned: Vec<u32>,
    pub apps: Vec<(u32, Vec<u8>)>,
}

const KEPT_MAGIC: &[u8; 4] = b"LSL1";
const KEPT_MAGIC_ACCOUNTS: &[u8; 4] = b"LSL2";

/// Several accounts' answers and when they were read, as a blob (`LSL2`, the time, how many
/// accounts, then each: its name's length and name, then its answers as [`encode_raw`] lays them
/// out after the time).
#[must_use]
pub fn encode_raws(raws: &[(String, Raw)], at_ms: u64) -> Vec<u8> {
    let n = |v: usize| u32::try_from(v).unwrap_or(u32::MAX).to_le_bytes();
    let mut b = Vec::new();
    b.extend_from_slice(KEPT_MAGIC_ACCOUNTS);
    b.extend_from_slice(&at_ms.to_le_bytes());
    b.extend_from_slice(&n(raws.len()));
    for (account, raw) in raws {
        b.extend_from_slice(&n(account.len()));
        b.extend_from_slice(account.as_bytes());
        let one = encode_raw(raw, 0);
        let body = one.get(12..).unwrap_or_default();
        b.extend_from_slice(&n(body.len()));
        b.extend_from_slice(body);
    }
    b
}

/// A blob's accounts' answers and when they were read: an `LSL2` one, or an `LSL1` one (a
/// single account's) as `single`'s; none when it is not a whole one.
#[must_use]
pub fn decode_raws(b: &[u8], single: Option<&str>) -> Option<(Vec<(String, Raw)>, u64)> {
    if b.get(..4)? == KEPT_MAGIC {
        let (raw, at) = decode_raw(b)?;
        return Some((vec![(single?.to_string(), raw)], at));
    }
    if b.get(..4)? != KEPT_MAGIC_ACCOUNTS {
        return None;
    }
    let mut at = 4usize;
    let mut take = |n: usize| {
        let s = b.get(at..at + n)?;
        at += n;
        Some(s)
    };
    let read_u32 = |s: &[u8]| s.try_into().ok().map(u32::from_le_bytes);
    let time = u64::from_le_bytes(take(8)?.try_into().ok()?);
    let count = read_u32(take(4)?)? as usize;
    let mut raws = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let len = read_u32(take(4)?)? as usize;
        let account = String::from_utf8(take(len)?.to_vec()).ok()?;
        let len = read_u32(take(4)?)? as usize;
        // Taken first: a length the blob does not hold allocates nothing.
        let body = take(len)?;
        let mut one = Vec::with_capacity(12 + len);
        one.extend_from_slice(KEPT_MAGIC);
        one.extend_from_slice(&0u64.to_le_bytes());
        one.extend_from_slice(body);
        raws.push((account, decode_raw(&one)?.0));
    }
    (at == b.len()).then_some((raws, time))
}

/// Steam's answers and when they were read, as a blob (`LSL1`, the time, the owned depots, each app's id and info).
#[must_use]
pub fn encode_raw(raw: &Raw, at_ms: u64) -> Vec<u8> {
    let n = |v: usize| u32::try_from(v).unwrap_or(u32::MAX).to_le_bytes();
    let mut b = Vec::new();
    b.extend_from_slice(KEPT_MAGIC);
    b.extend_from_slice(&at_ms.to_le_bytes());
    b.extend_from_slice(&n(raw.owned.len()));
    raw.owned.iter().for_each(|d| b.extend_from_slice(&d.to_le_bytes()));
    b.extend_from_slice(&n(raw.apps.len()));
    for (id, text) in &raw.apps {
        b.extend_from_slice(&id.to_le_bytes());
        b.extend_from_slice(&n(text.len()));
        b.extend_from_slice(text);
    }
    b
}

/// A blob's answers and when they were read; none when it is not a whole one.
#[must_use]
pub fn decode_raw(b: &[u8]) -> Option<(Raw, u64)> {
    let mut at = 0usize;
    let mut take = |n: usize| {
        let s = b.get(at..at + n)?;
        at += n;
        Some(s)
    };
    if take(4)? != KEPT_MAGIC {
        return None;
    }
    let read_u32 = |s: &[u8]| s.try_into().ok().map(u32::from_le_bytes);
    let time = u64::from_le_bytes(take(8)?.try_into().ok()?);
    let owned_n = read_u32(take(4)?)? as usize;
    let owned = (0..owned_n).map(|_| read_u32(take(4)?)).collect::<Option<Vec<u32>>>()?;
    let apps_n = read_u32(take(4)?)? as usize;
    let mut apps = Vec::with_capacity(apps_n.min(1 << 16));
    for _ in 0..apps_n {
        let id = read_u32(take(4)?)?;
        let len = read_u32(take(4)?)? as usize;
        apps.push((id, take(len)?.to_vec()));
    }
    (at == b.len()).then_some((Raw { owned, apps }, time))
}
