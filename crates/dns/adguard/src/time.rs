//! The times AdGuard Home's query log writes (RFC 3339: `2026-11-06T15:38:42.4098617Z`,
//! or with an offset such as `+04:00`), as ms since the epoch.

/// Days from 1970-01-01 to the date (proleptic Gregorian).
fn days(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn num(s: &str) -> Option<i64> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

/// `text` as ms since the epoch, if it is an RFC 3339 time after 1970.
#[must_use]
pub fn ms(text: &str) -> Option<u64> {
    let (date, rest) = text.split_once(['T', 't', ' '])?;
    let mut d = date.split('-');
    let (year, month, day) = (num(d.next()?)?, num(d.next()?)?, num(d.next()?)?);
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // The offset: Z, or +HH:MM / -HH:MM after the time.
    let (clock, offset_min) = if let Some(c) = rest.strip_suffix(['Z', 'z']) {
        (c, 0)
    } else {
        let at = rest.rfind(['+', '-'])?;
        let (c, off) = rest.split_at(at);
        let (h, m) = off[1..].split_once(':')?;
        let sign = if off.starts_with('-') { -1 } else { 1 };
        (c, sign * (num(h)? * 60 + num(m)?))
    };
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
    let mut t = hms.split(':');
    let (h, m, s) = (num(t.next()?)?, num(t.next()?)?, num(t.next()?)?);
    if t.next().is_some() || h > 23 || m > 59 || s > 60 {
        return None;
    }
    let millis = if frac.is_empty() {
        0
    } else {
        let digits: alloc::string::String = frac.chars().chain(core::iter::repeat('0')).take(3).collect();
        num(&digits)?
    };
    let secs = days(year, month, day) * 86_400 + h * 3_600 + m * 60 + s - offset_min * 60;
    u64::try_from(secs * 1_000 + millis).ok()
}
