//! Header fields the classifier and the list layer read.

use mail_parser::{HeaderName, Message};

pub(super) fn from(msg: &Message<'_>) -> (Option<String>, Option<String>) {
    let Some(addr) = msg.from().and_then(|a| a.first()) else {
        return (None, None);
    };
    let address = addr
        .address()
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty());
    let name = addr
        .name()
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty());
    (address, name)
}

/// A header's raw value, unfolded and trimmed.
pub(super) fn raw(msg: &Message<'_>, name: &str) -> Option<String> {
    let value = unfold(msg.header_raw(name)?);
    (!value.is_empty()).then_some(value)
}

fn unfold(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\n', "")
        .trim()
        .to_owned()
}

pub(super) fn list_id(msg: &Message<'_>) -> Option<String> {
    let value = raw(msg, "List-Id")?;
    let id = match (value.rfind('<'), value.rfind('>')) {
        (Some(open), Some(close)) if open < close => &value[open + 1..close],
        _ => value.as_str(),
    };
    let id = id.trim();
    (!id.is_empty()).then(|| id.to_owned())
}

pub(super) fn is_bulk(msg: &Message<'_>) -> bool {
    raw(msg, "Precedence")
        .map(|p| matches!(p.to_ascii_lowercase().as_str(), "bulk" | "list"))
        .unwrap_or(false)
}

/// Every DKIM `d=` tag, lowercased, in header order.
pub(super) fn dkim_domains(msg: &Message<'_>) -> Vec<String> {
    let raw_message = msg.raw_message();
    msg.headers()
        .iter()
        .filter(|h| h.name == HeaderName::DkimSignature)
        .filter_map(|h| {
            let bytes = raw_message.get(h.offset_start as usize..h.offset_end as usize)?;
            let value = unfold(&String::from_utf8_lossy(bytes));
            value.split(';').find_map(|tag| {
                let (key, val) = tag.split_once('=')?;
                (key.trim() == "d")
                    .then(|| val.split_whitespace().collect::<String>().to_lowercase())
            })
        })
        .filter(|d| !d.is_empty())
        .collect()
}

/// `Date` as RFC 3339 in UTC.
pub(super) fn date(msg: &Message<'_>) -> Option<String> {
    let date = msg.date()?;
    if !date.is_valid() {
        return None;
    }
    Some(rfc3339_utc(date.to_timestamp()))
}

/// Seconds since the epoch as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn rfc3339_utc(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let secs = timestamp.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

/// The inverse of [`rfc3339_utc`], for dates this crate wrote.
pub fn parse_rfc3339_utc(value: &str) -> Option<i64> {
    let b = value.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| value.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, min, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + min * 60 + s)
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_format_as_utc() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_772_357_400), "2026-03-01T09:30:00Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn parsing_inverts_formatting() {
        for t in [0, 951_782_400, 1_772_357_400, -86_400, 4_102_444_799] {
            assert_eq!(parse_rfc3339_utc(&rfc3339_utc(t)), Some(t));
        }
        assert_eq!(parse_rfc3339_utc("2026-03-01"), None);
    }
}
