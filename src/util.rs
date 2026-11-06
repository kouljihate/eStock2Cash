//! Small shared helpers: escaping, money/date formatting, ids.

use chrono::{DateTime, NaiveDate, Utc};

/// Text of an HTML fragment, tags removed — for `title`/`aria-label` attributes
/// that must not contain the `.bi` markup.
pub fn text_of(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut depth = 0usize;
    for ch in html.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
}

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Percent-encode a value for use in a query string.
pub fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

pub fn new_id() -> String {
    mongodb::bson::oid::ObjectId::new().to_hex()
}

/// Round to 2 decimals (money).
pub fn round2(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    (v * 100.0).round() / 100.0
}

/// 1234.5 -> "1234.50" (with thousands separators).
pub fn money(v: f64) -> String {
    let v = round2(v);
    let neg = v < 0.0;
    let v = v.abs();
    let whole = v.trunc() as i64;
    let cents = ((v - whole as f64) * 100.0).round() as i64;
    let mut s = whole.to_string();
    let mut grouped = String::new();
    let rev = s.chars().rev().collect::<Vec<_>>();
    for (i, ch) in rev.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*ch);
    }
    s = grouped.chars().rev().collect();
    format!("{}{}.{:02}", if neg { "-" } else { "" }, s, cents)
}

/// Format a decimal quantity without trailing noise: 3 -> "3", 3.5 -> "3.5".
pub fn qty(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(s, "%d/%m/%Y").ok())
}

/// Days between `date` (YYYY-MM-DD) and today. Positive = in the past.
pub fn days_since(date: &str) -> i64 {
    match parse_date(date) {
        Some(d) => (Utc::now().date_naive() - d).num_days(),
        None => 0,
    }
}

/// Days until `date`. Negative = overdue.
pub fn days_until(date: &str) -> i64 {
    -days_since(date)
}

pub fn add_days(date: &str, days: i64) -> String {
    parse_date(date)
        .map(|d| (d + chrono::Duration::days(days)).format("%Y-%m-%d").to_string())
        .unwrap_or_else(today)
}

pub fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

/// Constant-time byte comparison.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn if_empty(v: &str, fallback: &str) -> String {
    if v.trim().is_empty() {
        fallback.to_string()
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_groups_thousands() {
        assert_eq!(money(1234567.5), "1,234,567.50");
        assert_eq!(money(-2.345), "-2.35");
        assert_eq!(money(0.0), "0.00");
    }

    #[test]
    fn qty_trims_zeros() {
        assert_eq!(qty(3.0), "3");
        assert_eq!(qty(2.5), "2.5");
    }

    #[test]
    fn escapes_html() {
        assert_eq!(esc("<b>\"x\"</b>"), "&lt;b&gt;&quot;x&quot;&lt;/b&gt;");
    }
}
