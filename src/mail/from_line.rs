use std::fmt::{Display, Write};

use chrono::{DateTime, Local, TimeZone};

#[cfg(test)]
mod tests;

/// Generate a From_ line for mbox format.
///
/// Format: "From sender date\n"
/// Date is in ctime format: "Mon Jan  1 00:00:00 2024"
///
/// # Panics
/// Panics if sender is empty or contains whitespace/newlines.
pub fn generate(sender: &str) -> Vec<u8> {
    generate_with_time(sender, Local::now())
}

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
        !sender
            .bytes()
            .any(|b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r'),
        "sender must not contain whitespace"
    );

    // "From " (5) + sender + "  " (2) + ctime (24) + "\n" (1) = 32 + sender.len()
    let mut line = String::with_capacity(32 + sender.len());
    line.push_str("From ");
    line.push_str(sender);
    // Two spaces before date, matching procmail's ctime2buf2()
    line.push_str("  ");
    write!(line, "{}", time.format("%a %b %e %H:%M:%S %Y")).unwrap();
    line.push('\n');
    line.into_bytes()
}

/// Check if data starts with a From_ line.
pub fn starts_with_from(data: &[u8]) -> bool {
    data.starts_with(b"From ")
}

/// Skip From_ line(s) at start of data.
/// Also skips >From_ continuation lines (for forwarded mail).
pub fn skip_from_lines(mut data: &[u8]) -> &[u8] {
    while starts_with_from(data) || data.starts_with(b">From ") {
        if let Some(pos) = data.iter().position(|&b| b == b'\n') {
            data = &data[pos + 1..];
        } else {
            return &[];
        }
    }
    data
}
