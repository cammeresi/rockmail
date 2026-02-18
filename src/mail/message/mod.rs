use std::borrow::Cow;
use std::io::{self, Write};
use std::str;

use crate::field::{self, Field, FieldList};

#[cfg(test)]
mod tests;

/// Strip `\r\n` → `\n` in place using read/write cursors.
fn strip_cr_inplace(v: &mut Vec<u8>) {
    let Some(first) = v.iter().position(|&b| b == b'\r') else {
        return;
    };
    let mut w = first;
    let mut r = first;
    while r < v.len() {
        if v[r] == b'\r' && r + 1 < v.len() && v[r + 1] == b'\n' {
            v[w] = b'\n';
            w += 1;
            r += 2;
        } else {
            v[w] = v[r];
            w += 1;
            r += 1;
        }
    }
    v.truncate(w);
}

fn find_boundary(data: &[u8]) -> (usize, usize) {
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\n' && i + 1 < data.len() && data[i + 1] == b'\n' {
            return (i + 1, i + 2);
        }
        i += 1;
    }
    (data.len(), data.len())
}

fn unfold_value(data: &[u8]) -> Cow<'_, str> {
    let Some(pos) = data.iter().position(|&b| b == b'\n') else {
        return String::from_utf8_lossy(data);
    };
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..pos]);
    let mut i = pos;
    while i < data.len() {
        if data[i] == b'\n' && i + 1 < data.len() {
            out.push(b' ');
            i += 1;
            while i < data.len() && (data[i] == b' ' || data[i] == b'\t') {
                i += 1;
            }
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    Cow::Owned(String::from_utf8_lossy(&out).into_owned())
}

/// An email message with decomposed headers.
///
/// Headers are stored as a `FieldList` for direct mutation.
/// The body is a zero-copy slice into the original input.
#[derive(Debug, Clone)]
pub struct Message {
    fields: FieldList,
    raw: Vec<u8>,
    body_start: usize,
}

impl Message {
    /// Parse a message from raw bytes.
    ///
    /// CRLF line endings are normalized to LF.
    /// Leading blank lines before headers are skipped.
    pub fn parse(input: &[u8]) -> Self {
        let mut v = input.to_vec();
        strip_cr_inplace(&mut v);
        Self::parse_vec(v)
    }

    fn parse_vec(v: Vec<u8>) -> Self {
        let skip = v.iter().take_while(|&&b| b == b'\n').count();
        let (header_end, body_start) = find_boundary(&v[skip..]);
        let fields = field::parse_bytes(&v[skip..skip + header_end]);
        Self {
            fields,
            body_start: skip + body_start,
            raw: v,
        }
    }

    /// Create a message from pre-split header and body parts.
    ///
    /// Both `header` and `body` must use LF line endings (not CRLF).
    /// Use [`Message::parse`] if the input may contain CRLF.
    pub fn from_parts(header: &[u8], body: &[u8]) -> Self {
        let fields = field::parse_bytes(header);
        Self {
            fields,
            raw: body.to_vec(),
            body_start: 0,
        }
    }

    /// Create a message from a field list and body.
    pub fn from_fields(fields: FieldList, body: Vec<u8>) -> Self {
        Self {
            fields,
            raw: body,
            body_start: 0,
        }
    }

    /// Serialize headers to bytes.
    #[cfg(test)]
    pub fn header(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.fields.byte_len());
        self.fields.write_to(&mut buf).expect("Vec write");
        buf
    }

    /// Borrow the parsed field list.
    pub fn fields(&self) -> &FieldList {
        &self.fields
    }

    /// Mutably borrow the parsed field list.
    pub fn fields_mut(&mut self) -> &mut FieldList {
        &mut self.fields
    }

    /// Body portion.
    pub fn body(&self) -> &[u8] {
        &self.raw[self.body_start..]
    }

    /// Whether the serialized message ends with `\n`.
    ///
    /// Accounts for the separator between headers and body.
    pub fn ends_with_newline(&self) -> bool {
        let body = self.body();
        if body.is_empty() {
            !self.fields.is_empty()
        } else {
            body.ends_with(b"\n")
        }
    }

    /// Whether the serialized message ends with `\n\n`.
    ///
    /// Accounts for the separator between headers and body.
    pub fn ends_with_blank_line(&self) -> bool {
        let body = self.body();
        let has_hdr = !self.fields.is_empty();
        match body.len() {
            0 => has_hdr,
            1 => body[0] == b'\n' && has_hdr,
            _ => body.ends_with(b"\n\n"),
        }
    }

    /// Total message length (headers + separator + body).
    ///
    /// Includes the From_ line if present; `write_to(_, true)` will
    /// produce fewer bytes.
    pub fn len(&self) -> usize {
        let sep = usize::from(!self.fields.is_empty());
        self.fields.byte_len() + sep + self.body().len()
    }

    /// Whether message is empty.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.body().is_empty()
    }

    /// Write entire message (headers + separator + body) to a writer.
    ///
    /// If `strip_from` is true, the From_ line is omitted.
    pub fn write_to<W>(&self, w: &mut W, strip_from: bool) -> io::Result<usize>
    where
        W: Write,
    {
        let skip =
            strip_from && self.fields.first().is_some_and(|f| f.is_from_line());
        let mut n = 0;
        for f in self.fields.iter().skip(usize::from(skip)) {
            w.write_all(f.as_bytes())?;
            n += f.len();
        }
        if n > 0 {
            w.write_all(b"\n")?;
            n += 1;
        }
        let body = self.body();
        w.write_all(body)?;
        n += body.len();
        Ok(n)
    }

    /// Iterator over parsed headers (skips From_ and >From_ lines).
    pub fn headers(
        &self,
    ) -> impl Iterator<Item = (Cow<'_, str>, Cow<'_, str>)> {
        self.fields.iter().filter_map(|f| {
            if f.is_from_line() || f.as_bytes().starts_with(b">From ") {
                return None;
            }
            let name = String::from_utf8_lossy(f.name());
            let mut val = f.value();
            while !val.is_empty() && (val[0] == b' ' || val[0] == b'\t') {
                val = &val[1..];
            }
            let val = unfold_value(val);
            Some((name, val))
        })
    }

    /// Find a header by name (case-insensitive).
    pub fn get_header(&self, name: &str) -> Option<Cow<'_, str>> {
        for (n, v) in self.headers() {
            if n.eq_ignore_ascii_case(name) {
                return Some(v);
            }
        }
        None
    }

    /// Get Content-Length header value if present and valid.
    pub fn content_length(&self) -> Option<usize> {
        self.get_header("Content-Length")
            .and_then(|v| v.trim().parse().ok())
    }

    /// Extract mbox From_ line if present (without trailing newline).
    pub fn from_line(&self) -> Option<&[u8]> {
        let f = self.fields.first()?;
        if !f.is_from_line() {
            return None;
        }
        let text = f.as_bytes();
        let end = text.iter().position(|&b| b == b'\n').unwrap_or(text.len());
        Some(&text[..end])
    }

    /// Extract timestamp from From_ line if present.
    pub fn envelope_timestamp(&self) -> Option<&str> {
        super::extract_timestamp(self.from_line()?)
    }

    /// Extract sender from From_ line if present.
    pub fn envelope_sender(&self) -> Option<&str> {
        let line = self.from_line()?;
        let rest = &line[5..];
        let end = rest.iter().position(|&b| b == b' ').unwrap_or(rest.len());
        str::from_utf8(&rest[..end]).ok()
    }

    fn upsert_from(&mut self, raw: Vec<u8>) {
        let f = Field::new(raw).expect("valid From_ line");
        if self.fields.first().is_some_and(|f| f.is_from_line()) {
            self.fields.replace_first(f);
        } else {
            self.fields.insert(0, f);
        }
    }

    /// Set or replace the From_ line with a new sender.
    pub fn set_envelope_sender(&mut self, sender: &str) {
        self.upsert_from(super::generate(sender));
    }

    /// Replace the sender in the From_ line, preserving the existing timestamp.
    pub fn refresh_envelope_sender(&mut self, sender: &str) {
        let Some(ts) = self.envelope_timestamp().map(str::to_owned) else {
            return self.set_envelope_sender(sender);
        };
        self.upsert_from(super::generate_raw(sender, &ts));
    }

    /// Strip the From_ line if present.
    pub fn strip_from_line(&mut self) {
        if self.fields.first().is_some_and(|f| f.is_from_line()) {
            self.fields.remove(0);
        }
    }
}
