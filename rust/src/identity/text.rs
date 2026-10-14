//! Parsers for the text forms that conversions accept (spec 5.5).

use alloc::string::String;
use alloc::vec::Vec;

/// Text with ASCII whitespace removed from both ends.
pub(crate) fn trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

/// A boolean's text forms, case-insensitively.
pub(crate) fn bool(s: &str) -> Option<bool> {
    match trim(s).to_ascii_lowercase().as_str() {
        "true" | "t" | "yes" | "y" | "on" | "1" => Some(true),
        "false" | "f" | "no" | "n" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// NaN and the infinities, case-insensitively.
pub(crate) fn special_number(s: &str) -> Option<f64> {
    let lower = trim(s).to_lowercase().replace('\u{2212}', "-");
    match lower.as_str() {
        "nan" | "+nan" | "-nan" => Some(f64::NAN),
        "inf" | "+inf" | "infinity" | "+infinity" | "∞" | "+∞" => Some(f64::INFINITY),
        "-inf" | "-infinity" | "-∞" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

/// A UUID's canonical form: grouped and lowercase.
pub(crate) fn uuid(s: &str) -> Option<String> {
    let lower = trim(s).to_ascii_lowercase();
    let mut t = lower.as_str();
    let mut grouped_only = false;
    if let Some(inner) = t.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        t = inner;
    } else if let Some(rest) = t.strip_prefix("urn:uuid:") {
        t = rest;
        grouped_only = true;
    }
    let hex: String = if t.len() == 36 {
        let b = t.as_bytes();
        if [8, 13, 18, 23].iter().any(|&i| b[i] != b'-') {
            return None;
        }
        t.chars().filter(|&c| c != '-').collect()
    } else if t.len() == 32 && !grouped_only {
        t.into()
    } else {
        return None;
    };
    if hex.len() != 32 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(alloc::format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

/// Hexadecimal digits, in either case, as bytes.
pub(crate) fn hex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        return None;
    }
    b.chunks(2)
        .map(|pair| {
            let digit = |c: u8| (c as char).to_digit(16);
            Some((digit(pair[0])? * 16 + digit(pair[1])?) as u8)
        })
        .collect()
}

/// Base64 (RFC 4648), in the standard or the URL-safe alphabet but not
/// both, with padding correct or absent.
pub(crate) fn base64(s: &str) -> Option<Vec<u8>> {
    let body = s.trim_end_matches('=');
    let padding = s.len() - body.len();
    if padding > 0 && (s.len() % 4 != 0 || padding > 2) {
        return None;
    }
    let standard = body.contains(['+', '/']);
    let url_safe = body.contains(['-', '_']);
    if standard && url_safe || body.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in body.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Digits at `b[i..i + n]` as a number.
fn digits(b: &[u8], i: usize, n: usize) -> Option<i64> {
    let s = b.get(i..i + n)?;
    s.iter()
        .all(u8::is_ascii_digit)
        .then(|| s.iter().fold(0, |acc, d| acc * 10 + i64::from(d - b'0')))
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The nanoseconds of a fraction of a second, `.d` with 1 to 9 digits,
/// starting at `b[i]`; and the index after it.
fn fraction(b: &[u8], mut i: usize) -> Result<(i128, usize), &'static str> {
    if b.get(i) != Some(&b'.') {
        return Ok((0, i));
    }
    i += 1;
    let start = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let n = i - start;
    if n == 0 || n > 9 {
        return Err("a fraction of a second has 1 to 9 digits");
    }
    let value = digits(b, start, n).ok_or("bad fraction")?;
    Ok((i128::from(value) * 10i128.pow((9 - n) as u32), i))
}

/// An RFC 3339 date-time (spec 5.5) as nanoseconds since the Unix epoch.
pub(crate) fn instant(s: &str) -> Result<i128, &'static str> {
    const SHAPE: &str = "not an RFC 3339 date-time with an offset";
    let b = trim(s).as_bytes();
    let y = digits(b, 0, 4).ok_or(SHAPE)?;
    let mo = digits(b, 5, 2).ok_or(SHAPE)?;
    let d = digits(b, 8, 2).ok_or(SHAPE)?;
    let h = digits(b, 11, 2).ok_or(SHAPE)?;
    let mi = digits(b, 14, 2).ok_or(SHAPE)?;
    let sec = digits(b, 17, 2).ok_or(SHAPE)?;
    if b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't' | b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return Err(SHAPE);
    }
    let (nanos, mut i) = fraction(b, 19)?;
    let offset = match b.get(i) {
        Some(b'Z' | b'z') => {
            i += 1;
            0
        }
        Some(&sign @ (b'+' | b'-')) => {
            let oh = digits(b, i + 1, 2).ok_or(SHAPE)?;
            i += 3;
            let om = if b.get(i) == Some(&b':') {
                i += 3;
                digits(b, i - 2, 2).ok_or(SHAPE)?
            } else if i < b.len() {
                i += 2;
                digits(b, i - 2, 2).ok_or(SHAPE)?
            } else {
                0
            };
            if oh > 23 || om > 59 {
                return Err("offset out of range");
            }
            (oh * 3600 + om * 60) * if sign == b'-' { -1 } else { 1 }
        }
        _ => return Err("a date-time needs an offset"),
    };
    if i != b.len() {
        return Err(SHAPE);
    }
    if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || h > 23 || mi > 59 {
        return Err("date or time out of range");
    }
    if sec >= 60 {
        return Err("second out of range; leap seconds are errors");
    }
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec - offset;
    Ok(i128::from(secs) * 1_000_000_000 + nanos)
}

/// An ISO 8601 duration without years or months (spec 5.5), as
/// nanoseconds.
pub(crate) fn duration(s: &str) -> Result<i128, &'static str> {
    const SHAPE: &str = "not an ISO 8601 duration without years or months";
    let b = trim(s).as_bytes();
    let mut i = 0;
    let negative = match b.first() {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    if !matches!(b.get(i), Some(b'P' | b'p')) {
        return Err(SHAPE);
    }
    i += 1;
    let mut total: i128 = 0;
    let mut components = 0;
    let mut in_time = false;
    // Designators in the order they must appear, with their seconds.
    let date_units: &[(u8, i128)] = &[(b'W', 7 * 86_400), (b'D', 86_400)];
    let time_units: &[(u8, i128)] = &[(b'H', 3600), (b'M', 60), (b'S', 1)];
    let mut next = 0;
    while i < b.len() {
        if matches!(b[i], b'T' | b't') {
            if in_time {
                return Err(SHAPE);
            }
            in_time = true;
            next = 0;
            i += 1;
            if i == b.len() {
                return Err(SHAPE);
            }
            continue;
        }
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start || i - start > 18 {
            return Err(SHAPE);
        }
        let count = i128::from(digits(b, start, i - start).ok_or(SHAPE)?);
        let (nanos, after) = fraction(b, i)?;
        let has_fraction = after != i;
        i = after;
        let designator = b.get(i).ok_or(SHAPE)?.to_ascii_uppercase();
        i += 1;
        let units = if in_time { time_units } else { date_units };
        let Some(pos) = units[next..].iter().position(|(c, _)| *c == designator) else {
            return Err(SHAPE);
        };
        let (_, seconds) = units[next + pos];
        next += pos + 1;
        if has_fraction && designator != b'S' {
            return Err("only seconds may have a fraction");
        }
        total += count * seconds * 1_000_000_000 + nanos;
        components += 1;
    }
    if components == 0 {
        return Err(SHAPE);
    }
    Ok(if negative { -total } else { total })
}
