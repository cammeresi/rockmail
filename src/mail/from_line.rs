use std::fmt::Display;

use chrono::{DateTime, Local, TimeZone};

/// Generate a From_ line for mbox format.
///
/// Format: "From sender date\n"
/// Date is in ctime format: "Mon Jan  1 00:00:00 2024"
pub fn generate(sender: &str) -> Vec<u8> {
    generate_with_time(sender, Local::now())
}

/// Generate From_ line with explicit timestamp.
pub fn generate_with_time<Tz>(sender: &str, time: DateTime<Tz>) -> Vec<u8>
where
    Tz: TimeZone,
    Tz::Offset: Display,
{
    let mut line = Vec::with_capacity(64);
    line.extend_from_slice(b"From ");
    line.extend_from_slice(sender.as_bytes());
    line.push(b' ');
    // ctime format: "Mon Jan  1 00:00:00 2024"
    line.extend_from_slice(
        time.format("%a %b %e %H:%M:%S %Y").to_string().as_bytes(),
    );
    line.push(b'\n');
    line
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn generate_from_line() {
        let line = generate("user@example.com");
        assert!(line.starts_with(b"From user@example.com "));
        assert!(line.ends_with(b"\n"));
    }

    #[test]
    fn starts_with() {
        assert!(starts_with_from(
            b"From user@host Mon Jan 1 00:00:00 2024\n"
        ));
        assert!(!starts_with_from(b"From: user@host\n"));
        assert!(!starts_with_from(b"Subject: test\n"));
    }

    #[test]
    fn skip_from() {
        let data = b"From user Mon Jan 1 00:00:00 2024\nSubject: Test\n\nBody";
        let rest = skip_from_lines(data);
        assert!(rest.starts_with(b"Subject:"));
    }

    #[test]
    fn skip_multiple_from() {
        let data = b"From user Mon Jan 1 00:00:00 2024\n>From forwarded\nSubject: Test\n";
        let rest = skip_from_lines(data);
        assert!(rest.starts_with(b"Subject:"));
    }

    #[test]
    fn ctime_epoch() {
        let epoch = Utc.timestamp_opt(0, 0).unwrap();
        let line = generate_with_time("test", epoch);
        let s = String::from_utf8_lossy(&line);
        assert!(s.contains("Thu Jan  1 00:00:00 1970"), "got: {}", s);
    }
}
