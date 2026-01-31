use std::borrow::Cow;

/// Represents an email message with header/body separation.
///
/// Stores raw bytes and tracks the boundary between headers and body.
/// The body starts after the first blank line (two consecutive newlines).
#[derive(Debug, Clone)]
pub struct Message {
    data: Vec<u8>,
    /// End of header portion (exclusive).
    header_end: usize,
    /// Start of body portion.
    body_start: usize,
}

impl Message {
    /// Parse a message from raw bytes.
    ///
    /// Finds the header/body boundary (first `\n\n`) and records it.
    /// Leading blank lines before headers are skipped.
    pub fn parse(input: &[u8]) -> Self {
        let start = skip_leading_newlines(input);
        let input = &input[start..];

        let (header_end, body_start) = find_boundary(input);

        Self {
            data: input.to_vec(),
            header_end,
            body_start,
        }
    }

    /// Create a message from pre-split header and body parts.
    pub fn from_parts(header: &[u8], body: &[u8]) -> Self {
        let mut data = Vec::with_capacity(header.len() + 1 + body.len());
        data.extend_from_slice(header);
        if !header.is_empty() && !header.ends_with(b"\n") {
            data.push(b'\n');
        }
        let header_end = data.len();
        data.extend_from_slice(body);
        Self {
            data,
            header_end,
            body_start: header_end,
        }
    }

    /// Raw bytes of entire message.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Header portion (everything before blank line separator).
    pub fn header(&self) -> &[u8] {
        &self.data[..self.header_end]
    }

    /// Body portion (everything after blank line separator).
    pub fn body(&self) -> &[u8] {
        &self.data[self.body_start..]
    }

    /// Total message length.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether message is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Iterator over parsed headers.
    pub fn headers(&self) -> HeaderIter<'_> {
        HeaderIter {
            data: self.header(),
            pos: 0,
        }
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

    /// Extract mbox From_ line if present.
    ///
    /// The From_ line is not a real header - it's an mbox envelope marker
    /// in the format "From sender date".
    pub fn from_line(&self) -> Option<&[u8]> {
        let h = self.header();
        if h.starts_with(b"From ") {
            let end = h.iter().position(|&b| b == b'\n').unwrap_or(h.len());
            Some(&h[..end])
        } else {
            None
        }
    }

    /// Extract sender from From_ line if present.
    pub fn envelope_sender(&self) -> Option<&str> {
        let line = self.from_line()?;
        // Format: "From sender date..."
        // Skip "From " prefix, take until whitespace
        let rest = &line[5..];
        let end = rest.iter().position(|&b| b == b' ').unwrap_or(rest.len());
        std::str::from_utf8(&rest[..end]).ok()
    }
}

/// Iterates over headers, handling continuation lines.
pub struct HeaderIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for HeaderIter<'a> {
    type Item = (Cow<'a, str>, Cow<'a, str>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data.len() {
            return None;
        }

        let start = self.pos;
        let mut end = start;

        // Find end of header field (including continuation lines)
        loop {
            // Find end of current line
            while end < self.data.len() && self.data[end] != b'\n' {
                end += 1;
            }
            // Skip the newline
            if end < self.data.len() {
                end += 1;
            }
            // Check for continuation (next line starts with space/tab)
            if end < self.data.len()
                && (self.data[end] == b' ' || self.data[end] == b'\t')
            {
                continue;
            }
            break;
        }

        self.pos = end;
        let field = &self.data[start..end];

        // Skip From_ line (mbox envelope, not a header)
        if field.starts_with(b"From ") {
            return self.next();
        }

        parse_header_field(field)
    }
}

fn parse_header_field(field: &[u8]) -> Option<(Cow<'_, str>, Cow<'_, str>)> {
    let colon = field.iter().position(|&b| b == b':')?;
    let name = &field[..colon];
    let mut value = &field[colon + 1..];

    // Trim trailing newline
    if value.ends_with(b"\n") {
        value = &value[..value.len() - 1];
    }

    // Unfold continuation lines: replace \n followed by whitespace with single space
    let name = String::from_utf8_lossy(name);
    let value = unfold_header(value);

    Some((name, value))
}

fn unfold_header(data: &[u8]) -> Cow<'_, str> {
    // Fast path: no newlines means no unfolding needed
    if !data.contains(&b'\n') {
        return String::from_utf8_lossy(data);
    }

    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\n' && i + 1 < data.len() {
            // Replace newline + whitespace with single space
            result.push(b' ');
            i += 1;
            // Skip leading whitespace on continuation line
            while i < data.len() && (data[i] == b' ' || data[i] == b'\t') {
                i += 1;
            }
        } else {
            result.push(data[i]);
            i += 1;
        }
    }

    Cow::Owned(String::from_utf8_lossy(&result).into_owned())
}

/// Skip leading blank lines.
fn skip_leading_newlines(data: &[u8]) -> usize {
    data.iter().take_while(|&&b| b == b'\n').count()
}

/// Find header/body boundary.
/// Returns (header_end, body_start).
/// header_end is position after last header byte (before blank line separator).
/// body_start is position of first body byte (after blank line separator).
fn find_boundary(data: &[u8]) -> (usize, usize) {
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\n' {
            // Check for blank line
            if i + 1 < data.len() && data[i + 1] == b'\n' {
                // header ends at i+1 (include the first \n)
                // body starts at i+2 (after the second \n)
                return (i + 1, i + 2);
            }
        }
        i += 1;
    }
    // No blank line found - everything is header, no body
    (data.len(), data.len())
}

#[cfg(test)]
mod tests;
