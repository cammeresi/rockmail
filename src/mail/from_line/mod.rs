use std::fmt::{Display, Write};
use std::str;

use chrono::{DateTime, Local, TimeZone};

#[cfg(test)]
mod tests;

/// Generate From_ line with explicit timestamp.
///
/// # Panics
/// Panics if sender is empty or contains whitespace/newlines.
pub fn generate_with_time<Tz>(sender: &str, time: DateTime<Tz>) -> Vec<u8>
where
    Tz: TimeZone,
    Tz::Offset: Display,
{
    assert!(!sender.is_empty(), "sender must not be empty");
    assert!(
        !sender.bytes().any(|b| b.is_ascii_whitespace()),
        "sender must not contain whitespace"
    );

    // "From " (5) + sender + "  " (2) + ctime (24) + "\n" (1) = 32 +
    // sender.len()
    let mut line = String::with_capacity(32 + sender.len());
    line.push_str("From ");
    line.push_str(sender);
    // Two spaces before date, matching procmail's ctime2buf2()
    line.push_str("  ");
    write!(line, "{}", time.format("%a %b %e %H:%M:%S %Y")).unwrap();
    line.push('\n');
    line.into_bytes()
}

/// Generate a From_ line for mbox format.
///
/// Format: "From sender  date\n"
/// Date is in ctime format: "Mon Jan  1 00:00:00 2024"
///
/// # Panics
/// Panics if sender is empty or contains whitespace/newlines.
pub fn generate(sender: &str) -> Vec<u8> {
    generate_with_time(sender, Local::now())
}

/// Generate a From_ line with a raw timestamp string.
///
/// Format: "From sender  timestamp\n"
pub fn generate_raw(sender: &str, timestamp: &str) -> Vec<u8> {
    let mut line =
        Vec::with_capacity(5 + sender.len() + 2 + timestamp.len() + 1);
    line.extend_from_slice(b"From ");
    line.extend_from_slice(sender.as_bytes());
    line.extend_from_slice(b"  ");
    line.extend_from_slice(timestamp.as_bytes());
    line.push(b'\n');
    line
}

/// Extract timestamp from a From_ line (without trailing newline).
///
/// Input: `From sender  Mon Jan  1 00:00:00 2024`
pub fn extract_timestamp(line: &[u8]) -> Option<&str> {
    let rest = line.strip_prefix(b"From ")?;
    let i = rest.iter().position(|&b| b == b' ')?;
    let j = rest[i..].iter().position(|&b| b != b' ')?;
    let ts = &rest[i + j..];
    if !ts.contains(&b':') {
        return None;
    }
    str::from_utf8(ts).ok()
}

/// Skip From_ line(s) at start of data.
/// Also skips >From_ continuation lines (for forwarded mail).
pub fn skip_from_lines(mut data: &[u8]) -> &[u8] {
    while data.starts_with(b"From ") || data.starts_with(b">From ") {
        if let Some(pos) = data.iter().position(|&b| b == b'\n') {
            data = &data[pos + 1..];
        } else {
            return &[];
        }
    }
    data
}
