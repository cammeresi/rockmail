//! Formail: mail header manipulation and mailbox splitting.
//!
//! This module provides the core functionality for the formail binary,
//! including header field operations, auto-reply generation, and mailbox
//! splitting.

#[cfg(test)]
mod tests;

use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::mem;
use std::ops::{Deref, DerefMut};

/// Find the end of the field name (position after the colon).
/// Returns None if this doesn't look like a valid header field.
fn find_field_name_end(data: &[u8]) -> Option<usize> {
    // From_ line is special
    if data.starts_with(b"From ") {
        return Some(5);
    }

    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b':' => return Some(i + 1),
            b' ' | b'\t' => {
                // Whitespace before colon is allowed
                let mut j = i + 1;
                while j < data.len() && (data[j] == b' ' || data[j] == b'\t') {
                    j += 1;
                }
                if j < data.len() && data[j] == b':' {
                    return Some(j + 1);
                }
                return None;
            }
            b'\n' | b'\r' => return None,
            c if c.is_ascii_control() => return None,
            _ => i += 1,
        }
    }
    None
}

/// A parsed email header field.
#[derive(Debug, Clone)]
pub struct Field {
    /// Full text of the field including name, colon, value, and newline.
    pub text: Vec<u8>,
    /// Length of field name including colon (and any whitespace before colon).
    pub name_len: usize,
}

impl Field {
    /// Create a field from raw text.
    pub fn new(text: Vec<u8>) -> Option<Self> {
        let name_len = find_field_name_end(&text)?;
        Some(Self { text, name_len })
    }

    /// Create a field from name and value.
    pub fn from_parts(name: &[u8], value: &[u8]) -> Self {
        let mut text = Vec::with_capacity(name.len() + value.len() + 2);
        text.extend_from_slice(name);
        if !name.ends_with(b":") {
            text.push(b':');
        }
        if !value.is_empty() && value[0] != b' ' && value[0] != b'\t' {
            text.push(b' ');
        }
        text.extend_from_slice(value);
        if !text.ends_with(b"\n") {
            text.push(b'\n');
        }
        let name_len = find_field_name_end(&text).unwrap_or(name.len() + 1);
        Self { text, name_len }
    }

    /// The field name (without colon).
    pub fn name(&self) -> &[u8] {
        let end = if self.name_len > 0 && self.text[self.name_len - 1] == b':' {
            self.name_len - 1
        } else {
            self.name_len
        };
        &self.text[..end]
    }

    /// The field value (after colon, without trailing newline).
    pub fn value(&self) -> &[u8] {
        let start = self.name_len;
        let end = self.text.len().saturating_sub(1);
        if start < end {
            &self.text[start..end]
        } else {
            &[]
        }
    }

    /// Whether this is a From_ line (mbox envelope).
    pub fn is_from_line(&self) -> bool {
        self.text.starts_with(b"From ")
    }

    /// Total length of the field.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Whether this field has no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Check if name matches (case-insensitive, prefix match allowed if
    /// pattern contains no colon).
    pub fn name_matches(&self, pattern: &[u8]) -> bool {
        let name = self.name();
        let pat = pattern.strip_suffix(b":").unwrap_or(pattern);
        if pat.len() > name.len() {
            return false;
        }
        name[..pat.len()].eq_ignore_ascii_case(pat)
            && (pat.len() == name.len() || !pat.contains(&b':'))
    }

    /// Rename this field by changing the name portion.
    pub fn rename(&mut self, new_name: &[u8]) {
        let old_value_start = self.name_len;
        let mut new_text = Vec::with_capacity(
            new_name.len() + self.text.len() - old_value_start,
        );
        new_text.extend_from_slice(new_name);
        if !new_name.ends_with(b":")
            && old_value_start > 0
            && self.text[old_value_start - 1] == b':'
        {
            new_text.push(b':');
        }
        new_text.extend_from_slice(&self.text[old_value_start..]);
        self.name_len =
            find_field_name_end(&new_text).unwrap_or(new_name.len() + 1);
        self.text = new_text;
    }

    /// Concatenate continuation lines (replace newlines in value with spaces).
    pub fn concatenate(&mut self) {
        if self.is_from_line() {
            return;
        }
        let mut i = self.name_len;
        while i < self.text.len() {
            if self.text[i] == b'\n' && i + 1 < self.text.len() {
                self.text[i] = b' ';
            }
            i += 1;
        }
    }

    /// Check if this field is empty (has no value content).
    pub fn is_empty_value(&self) -> bool {
        if self.is_from_line() {
            return false;
        }
        self.value().is_empty()
    }

    /// Ensure there's a space after the colon. Returns true if field was
    /// modified.
    pub fn zap_whitespace(&mut self) -> bool {
        if self.is_from_line() {
            return false;
        }
        // Check if there's content after the colon
        if self.name_len >= self.text.len() {
            return false;
        }
        let after_colon = self.text[self.name_len];
        if after_colon != b' ' && after_colon != b'\t' && after_colon != b'\n' {
            // Insert a space after the colon
            self.text.insert(self.name_len, b' ');
            true
        } else {
            false
        }
    }
}

/// A list of header fields.
#[derive(Debug, Default)]
pub struct FieldList {
    fields: Vec<Field>,
}

impl Deref for FieldList {
    type Target = Vec<Field>;

    fn deref(&self) -> &Self::Target {
        &self.fields
    }
}

impl DerefMut for FieldList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields
    }
}

impl FieldList {
    /// Create an empty field list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Find first field matching the pattern (case-insensitive prefix).
    pub fn find(&self, pattern: &[u8]) -> Option<&Field> {
        self.fields.iter().find(|f| f.name_matches(pattern))
    }

    /// Find first field matching the pattern, returning mutable reference.
    pub fn find_mut(&mut self, pattern: &[u8]) -> Option<&mut Field> {
        self.fields.iter_mut().find(|f| f.name_matches(pattern))
    }

    /// Remove all fields matching the pattern.
    pub fn remove_all(&mut self, pattern: &[u8]) {
        self.fields.retain(|f| !f.name_matches(pattern));
    }

    /// Keep only the first occurrence of fields matching pattern.
    pub fn keep_first(&mut self, pattern: &[u8]) {
        let mut seen = false;
        self.fields.retain(|f| {
            if f.name_matches(pattern) {
                if seen {
                    return false;
                }
                seen = true;
            }
            true
        });
    }

    /// Keep only the last occurrence of fields matching pattern.
    pub fn keep_last(&mut self, pattern: &[u8]) {
        let last_idx =
            self.fields.iter().rposition(|f| f.name_matches(pattern));
        if let Some(last) = last_idx {
            let mut idx = 0;
            self.fields.retain(|f| {
                let keep = !f.name_matches(pattern) || idx == last;
                idx += 1;
                keep
            });
        }
    }

    /// Rename all fields matching old_name to new_name.
    pub fn rename_all(&mut self, old_name: &[u8], new_name: &[u8]) {
        for field in &mut self.fields {
            if field.name_matches(old_name) {
                field.rename(new_name);
            }
        }
    }

    /// Prepend "Old-" to all fields matching pattern.
    pub fn prepend_old(&mut self, pattern: &[u8]) {
        for field in &mut self.fields {
            if field.name_matches(pattern) {
                let mut new_name = b"Old-".to_vec();
                new_name.extend_from_slice(field.name());
                new_name.push(b':');
                field.rename(&new_name);
            }
        }
    }

    /// Write all fields to a writer.
    pub fn write_to<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: Write,
    {
        for field in &self.fields {
            w.write_all(&field.text)?;
        }
        Ok(())
    }

    /// Zap whitespace: ensure space after colon and remove empty fields.
    pub fn zap_whitespace(&mut self) {
        for field in &mut self.fields {
            field.zap_whitespace();
        }
        self.fields.retain(|f| !f.is_empty_value());
    }
}

/// Parse header fields from a byte slice.
pub fn parse_bytes(header: &[u8]) -> FieldList {
    let (fields, _) = read_headers(Cursor::new(header)).unwrap_or_default();
    fields
}

/// Read header fields from a reader.
/// Stops at the first blank line (end of headers).
pub fn read_headers<R>(reader: R) -> io::Result<(FieldList, Vec<u8>)>
where
    R: Read,
{
    let mut reader = BufReader::new(reader);
    let mut fields = FieldList::new();
    let mut buf = Vec::new();
    let mut leftover = Vec::new();

    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }

        // Blank line ends header
        if buf == b"\n" || buf == b"\r\n" {
            break;
        }

        // Check if this looks like a header field.
        // From_ only counts as a header if it's the very first field.
        if let Some(name_len) = find_field_name_end(&buf)
            && (!buf.starts_with(b"From ") || fields.is_empty())
        {
            // Read continuation lines
            loop {
                let mut peek = [0u8; 1];
                match reader.fill_buf() {
                    Ok(available) if !available.is_empty() => {
                        peek[0] = available[0];
                    }
                    _ => break,
                }
                if peek[0] == b' ' || peek[0] == b'\t' {
                    reader.read_until(b'\n', &mut buf)?;
                } else {
                    break;
                }
            }

            fields.push(Field {
                text: mem::take(&mut buf),
                name_len,
            });
        } else {
            // Not a header field - this is the start of body
            leftover = buf.clone();
            break;
        }
    }

    // Read remaining data
    reader.read_to_end(&mut leftover)?;
    Ok((fields, leftover))
}
