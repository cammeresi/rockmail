//! RFC 2047 encoded-word decoding and encoding.
//!
//! This is a rockmail extension beyond procmail compatibility.

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::fmt::Write;
use std::str;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use encoding_rs::Encoding;

// =?UTF-8?B?...?= — 10-char prefix + 2-char suffix = 12 overhead.
// RFC 2047 §2: encoded-word max 75 chars, so 63 chars for base64 payload,
// which encodes floor(63/4)*3 = 45 raw bytes.
const MAX_WORD: usize = 75;
const MAX_B_RAW: usize = 45;

/// Encoding type for RFC 2047 encoded words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enc {
    /// Base64 encoding.
    B,
    /// Quoted-Printable encoding.
    Q,
}

impl TryFrom<u8> for Enc {
    type Error = ();

    fn try_from(b: u8) -> Result<Self, ()> {
        match b {
            b'B' | b'b' => Ok(Enc::B),
            b'Q' | b'q' => Ok(Enc::Q),
            _ => Err(()),
        }
    }
}

impl Enc {
    /// Detect the encoding used in a raw header value.
    pub fn detect(raw: &[u8]) -> Option<Enc> {
        let start = find_pair(raw, b"=?")?;
        let rest = &raw[start + 2..];
        let q1 = find_byte(rest, b'?')?;
        let enc_byte = *rest.get(q1 + 1)?;
        Enc::try_from(enc_byte).ok()
    }
}

fn find_pair(haystack: &[u8], needle: &[u8; 2]) -> Option<usize> {
    haystack.windows(2).position(|w| w == needle)
}

fn find_byte(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

fn is_all_whitespace(s: &[u8]) -> bool {
    s.iter().all(u8::is_ascii_whitespace)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    Some((hex_val(hi)? << 4) | hex_val(lo)?)
}

fn convert_charset(charset: &[u8], raw: &[u8]) -> String {
    let name = str::from_utf8(charset).unwrap_or("UTF-8");
    let enc =
        Encoding::for_label(name.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    if enc == encoding_rs::UTF_8 {
        return String::from_utf8_lossy(raw).into_owned();
    }
    let (cow, _) = enc.decode_without_bom_handling(raw);
    cow.into_owned()
}

fn q_decode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'_' => out.push(b' '),
            b'=' if i + 2 < input.len() => {
                if let Some(v) = hex_pair(input[i + 1], input[i + 2]) {
                    out.push(v);
                    i += 2;
                } else {
                    out.push(b'=');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    out
}

/// Length of a single Q-encoded byte: 1 for literals, 3 for =XX.
fn q_byte_len(b: u8) -> usize {
    match b {
        b' '
        | b'!'
        | b'*'
        | b'+'
        | b'-'
        | b'/'
        | b'0'..=b'9'
        | b'A'..=b'Z'
        | b'a'..=b'z' => 1,
        _ => 3,
    }
}

fn q_push_byte(out: &mut String, b: u8) {
    match b {
        b' ' => out.push('_'),
        b'!'
        | b'*'
        | b'+'
        | b'-'
        | b'/'
        | b'0'..=b'9'
        | b'A'..=b'Z'
        | b'a'..=b'z' => out.push(b as char),
        _ => write!(out, "={b:02X}").unwrap(),
    }
}

#[cfg(test)]
fn q_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for &b in input {
        q_push_byte(&mut out, b);
    }
    out
}

/// Parse an encoded word starting at `=?`.
///
/// Returns (bytes_consumed, enc, decoded_text).
fn find_encoded_word(input: &[u8]) -> Option<(usize, Enc, String)> {
    if input.len() < 8 || &input[..2] != b"=?" {
        return None;
    }
    let rest = &input[2..];

    let q1 = find_byte(rest, b'?')?;
    let charset = &rest[..q1];

    let tail = &rest[q1 + 1..];
    let enc = Enc::try_from(*tail.first()?).ok()?;
    if tail.get(1) != Some(&b'?') {
        return None;
    }

    let payload = &tail[2..];
    let end = find_pair(payload, b"?=")?;
    let encoded = &payload[..end];

    let total = 2 + charset.len() + 1 + 1 + 1 + encoded.len() + 2;

    let raw = match enc {
        Enc::B => STANDARD.decode(encoded).ok()?,
        Enc::Q => q_decode(encoded),
    };

    let text = convert_charset(charset, &raw);
    Some((total, enc, text))
}

/// Find the largest split point <= `at` on a UTF-8 char boundary.
fn utf8_floor(s: &str, at: usize) -> usize {
    if at >= s.len() {
        return s.len();
    }
    // floor_char_boundary is nightly; do it manually
    let mut i = at;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    // If we rounded down to 0 but at > 0, include the whole first char
    if i == 0 && at > 0 {
        let c = s.chars().next().unwrap();
        c.len_utf8()
    } else {
        i
    }
}

fn encode_b(text: &str) -> String {
    let max = MAX_B_RAW;
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        let split = utf8_floor(rest, max);
        if !out.is_empty() {
            out.push_str("\r\n ");
        }
        out.push_str("=?UTF-8?B?");
        out.push_str(&STANDARD.encode(&rest.as_bytes()[..split]));
        out.push_str("?=");
        rest = &rest[split..];
    }
    out
}

fn encode_q(text: &str) -> String {
    let mut out = String::new();
    let mut line = String::from("=?UTF-8?Q?");
    let mut buf = [0u8; 4];
    for c in text.chars() {
        let bytes = c.encode_utf8(&mut buf).as_bytes();
        let clen: usize = bytes.iter().map(|&b| q_byte_len(b)).sum();
        if line.len() + clen + 2 > MAX_WORD {
            line.push_str("?=");
            if !out.is_empty() {
                out.push_str("\r\n ");
            }
            out.push_str(&line);
            line = String::from("=?UTF-8?Q?");
        }
        for &b in bytes {
            q_push_byte(&mut line, b);
        }
    }
    line.push_str("?=");
    if !out.is_empty() {
        out.push_str("\r\n ");
    }
    out.push_str(&line);
    out
}

/// Decode RFC 2047 encoded words in a header value.
///
/// Scans for `=?charset?encoding?text?=` tokens, decodes them to UTF-8.
/// Collapses whitespace between adjacent encoded words per RFC 2047 §6.2.
/// Invalid tokens pass through unchanged.
pub fn decode(input: &[u8]) -> Cow<'_, str> {
    if find_pair(input, b"=?").is_none() {
        return String::from_utf8_lossy(input);
    }
    let mut out = Vec::with_capacity(input.len());
    let mut pos = 0;
    let mut last_was_encoded = false;

    while pos < input.len() {
        let Some(start) = find_pair(&input[pos..], b"=?") else {
            out.extend_from_slice(&input[pos..]);
            break;
        };
        let abs = pos + start;

        // Collapse inter-word whitespace per §6.2
        if !last_was_encoded || !is_all_whitespace(&input[pos..abs]) {
            out.extend_from_slice(&input[pos..abs]);
        }

        if let Some((end, _, decoded)) = find_encoded_word(&input[abs..]) {
            out.extend_from_slice(decoded.as_bytes());
            pos = abs + end;
            last_was_encoded = true;
        } else {
            out.extend_from_slice(&input[abs..abs + 2]);
            pos = abs + 2;
            last_was_encoded = false;
        }
    }

    Cow::Owned(match String::from_utf8(out) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}

/// Encode a UTF-8 string as an RFC 2047 encoded word.
///
/// Folds into multiple encoded words at ~76 chars per line.
pub fn encode(text: &str, enc: Enc) -> String {
    if text.is_ascii() {
        return text.to_owned();
    }
    match enc {
        Enc::B => encode_b(text),
        Enc::Q => encode_q(text),
    }
}
