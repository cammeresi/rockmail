//! formail - mail (re)formatter
//!
//! Converts mail to mailbox format, performs From_ escaping, generates
//! auto-reply headers, header manipulation, and mailbox/digest splitting.

use std::fs::OpenOptions;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::mem;
use std::process::{Child, Command, ExitCode, Stdio};
use std::str;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use nix::fcntl::{Flock, FlockArg};

use rockmail::formail::{Field, FieldList, read_header};
use rockmail::util;
use rockmail::variables::Environment;

#[cfg(test)]
mod tests;

enum Quote {
    None,
    From,
    All,
}

impl Quote {
    fn prefix(&self, line: &[u8]) -> bool {
        match self {
            Quote::None => false,
            Quote::From => line.starts_with(b"From "),
            Quote::All => true,
        }
    }
}

#[derive(Default)]
struct SplitState {
    msg_num: usize,
    count: usize,
    msg: Vec<u8>,
    pending: Vec<u8>,
    fields: usize,
    last_blank: bool,
    in_msg: bool,
    content_remaining: usize,
}

impl SplitState {
    fn new() -> Self {
        Self {
            last_blank: true,
            ..Default::default()
        }
    }
}

struct SplitConfig<'a> {
    cmd: &'a [String],
    skip: usize,
    total: Option<usize>,
    minfields: usize,
    every: bool,
    digest: bool,
    base: i64,
    width: usize,
}

impl SplitConfig<'_> {
    fn is_boundary(&self, line: &[u8], last_blank: bool) -> bool {
        if self.digest || self.every {
            is_header_field(line) && (self.every || last_blank)
        } else {
            line.starts_with(b"From ") && last_blank
        }
    }
}

struct ProcessPool {
    children: Vec<Child>,
    max: usize,
}

impl ProcessPool {
    fn new(max: usize) -> Self {
        Self {
            children: Vec::new(),
            max,
        }
    }

    fn wait_one(&mut self) -> io::Result<()> {
        if let Some(mut child) = self.children.pop() {
            child.wait()?;
        }
        Ok(())
    }

    fn wait_all(&mut self) -> io::Result<()> {
        while !self.children.is_empty() {
            self.wait_one()?;
        }
        Ok(())
    }

    fn spawn(&mut self, child: Child) -> io::Result<()> {
        while self.children.len() >= self.max {
            self.wait_one()?;
        }
        self.children.push(child);
        Ok(())
    }
}

/// formail - mail (re)formatter
#[derive(Parser, Debug, Default)]
#[command(name = "formail", version, about, disable_version_flag = true)]
#[command(
    override_usage = "formail [+skip] [-total] [options] [-s [command [arg ...]]]"
)]
struct Args {
    /// Print version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: (),

    /// Skip the first N messages while splitting
    #[arg(short = '+', value_name = "skip", hide = true)]
    skip: Option<usize>,

    /// Output at most N messages while splitting (use -N syntax)
    #[arg(value_name = "total", hide = true)]
    total: Option<usize>,

    /// Don't escape bogus From_ lines
    #[arg(short = 'b')]
    no_escape: bool,

    /// Concatenate continued header fields
    #[arg(short = 'c')]
    concatenate: bool,

    /// Split digests (relaxed message boundary detection)
    #[arg(short = 'd')]
    digest: bool,

    /// Don't require empty lines before headers
    #[arg(short = 'e')]
    every: bool,

    /// Force pass-through (don't add From_ line)
    #[arg(short = 'f')]
    force: bool,

    /// Keep body when replying or extracting
    #[arg(short = 'k')]
    keep_body: bool,

    /// Generate procmail-style log summary
    #[arg(short = 'l', value_name = "folder")]
    log: Option<String>,

    /// Minimum header fields to recognize message start
    #[arg(short = 'm', value_name = "minfields")]
    minfields: Option<usize>,

    /// Don't wait for programs (parallel split), optional max procs (default 4)
    #[arg(short = 'n', num_args = 0..=1, default_missing_value = "4")]
    nowait: Option<usize>,

    /// Quotation prefix for From_ escaping (default ">")
    #[arg(short = 'p', value_name = "prefix")]
    prefix: Option<String>,

    /// Be quiet about errors (always on, ignored)
    #[arg(short = 'q')]
    quiet: bool,

    /// Generate auto-reply header
    #[arg(short = 'r')]
    reply: bool,

    /// Split into separate messages, pipe to command
    #[arg(short = 's', num_args = 0..)]
    split: Option<Vec<String>>,

    /// Trust header sender for replies (vs envelope sender)
    #[arg(short = 't')]
    trust: bool,

    /// Zap whitespace (ensure space after colon, remove empty fields)
    #[arg(short = 'z')]
    zap: bool,

    /// Detect duplicates using cache file
    #[arg(short = 'D', value_names = ["maxlen", "idcache"])]
    duplicate: Option<Vec<String>>,

    // Header operations (can be repeated)
    /// Add header if not present (use "Name:" for just name, generates
    /// Message-ID)
    #[arg(short = 'a', value_name = "header", action = clap::ArgAction::Append)]
    add_if_not: Vec<String>,

    /// Add header always
    #[arg(short = 'A', value_name = "header", action = clap::ArgAction::Append)]
    add_always: Vec<String>,

    /// Rename existing field to Old-*, then insert new
    #[arg(short = 'i', value_name = "header", action = clap::ArgAction::Append)]
    rename_insert: Vec<String>,

    /// Delete existing field, then insert new
    #[arg(short = 'I', value_name = "header", action = clap::ArgAction::Append)]
    delete_insert: Vec<String>,

    /// Rename field (oldname: newname:)
    #[arg(
        short = 'R',
        num_args = 2,
        value_names = ["old", "new"],
        action = clap::ArgAction::Append
    )]
    rename: Vec<String>,

    /// Keep first occurrence only
    #[arg(short = 'u', value_name = "field", action = clap::ArgAction::Append)]
    first_uniq: Vec<String>,

    /// Keep last occurrence only
    #[arg(short = 'U', value_name = "field", action = clap::ArgAction::Append)]
    last_uniq: Vec<String>,

    /// Extract field contents only
    #[arg(short = 'x', value_name = "field", action = clap::ArgAction::Append)]
    extract: Vec<String>,

    /// Extract field with name
    #[arg(short = 'X', value_name = "field", action = clap::ArgAction::Append)]
    extract_keep: Vec<String>,
}

/// Parse +N (skip) and -N (total) from beginning of args.
fn parse_skip_total(
    args: &[String],
) -> (Option<usize>, Option<usize>, Vec<String>) {
    let mut skip = None;
    let mut total = None;
    let mut rest = vec![args[0].clone()];
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];
        if let Some(n) = arg.strip_prefix('+')
            && let Ok(v) = n.parse::<usize>()
        {
            skip = Some(v);
            i += 1;
            continue;
        }
        if let Some(rest) = arg.strip_prefix('-')
            && !rest.starts_with('-')
            && let Some(c) = rest.chars().next()
            && c.is_ascii_digit()
            && let Ok(v) = arg[1..].parse::<usize>()
        {
            total = Some(v);
            i += 1;
            continue;
        }
        break;
    }

    rest.extend(args[i..].iter().cloned());
    (skip, total, rest)
}

fn parse_header_arg(s: &str) -> (&str, &str) {
    if let Some(pos) = s.find(':') {
        let name = &s[..pos];
        let value = s[pos + 1..].trim_start();
        (name, value)
    } else {
        (s, "")
    }
}

fn parse_content_length(header: &[u8]) -> usize {
    const PREFIX: &[u8] = b"Content-Length:";
    for line in header.split(|&b| b == b'\n') {
        if line.len() > PREFIX.len()
            && line[..PREFIX.len()].eq_ignore_ascii_case(PREFIX)
        {
            let val = &line[PREFIX.len()..];
            let s = str::from_utf8(val).unwrap_or("");
            if let Ok(n) = s.trim().parse::<usize>() {
                return n;
            }
        }
    }
    0
}

fn is_header_field(line: &[u8]) -> bool {
    if line.starts_with(b"From ") {
        return true;
    }
    for (i, &b) in line.iter().enumerate() {
        match b {
            b':' => return i > 0,
            b' ' | b'\t' | b'\n' | b'\r' => return false,
            _ if b.is_ascii_control() => return false,
            _ => {}
        }
    }
    false
}

// Sender scoring table matching procmail's sest[] (formail.c:62-74).
// Array index = envelope weight; wrepl/wrrepl = reply weights.
struct SenderField {
    name: &'static [u8],
    reply_weight: i32,
    resent_reply_weight: i32,
}

const SEST: &[SenderField] = &[
    SenderField {
        name: b"Reply-To:",
        reply_weight: 8,
        resent_reply_weight: 7,
    },
    SenderField {
        name: b"From:",
        reply_weight: 7,
        resent_reply_weight: 6,
    },
    SenderField {
        name: b"Sender:",
        reply_weight: 6,
        resent_reply_weight: 5,
    },
    SenderField {
        name: b"Resent-Reply-To:",
        reply_weight: 0,
        resent_reply_weight: 10,
    },
    SenderField {
        name: b"Resent-From:",
        reply_weight: 0,
        resent_reply_weight: 9,
    },
    SenderField {
        name: b"Resent-Sender:",
        reply_weight: 0,
        resent_reply_weight: 8,
    },
    SenderField {
        name: b"Path:",
        reply_weight: 1,
        resent_reply_weight: 0,
    },
    SenderField {
        name: b"Return-Receipt-To:",
        reply_weight: 2,
        resent_reply_weight: 1,
    },
    SenderField {
        name: b"Errors-To:",
        reply_weight: 3,
        resent_reply_weight: 2,
    },
    SenderField {
        name: b"Return-Path:",
        reply_weight: 5,
        resent_reply_weight: 4,
    },
    SenderField {
        name: b"From ",
        reply_weight: 4,
        resent_reply_weight: 3,
    },
];

#[derive(Clone, Copy)]
enum SenderMode {
    Envelope,
    Reply,
    ResentReply,
}

/// Extract an RFC 822 address from a header value.
/// Prefers `<addr>` angle-bracket form, skips comments `(...)` and
/// quoted strings, falls back to first non-comment word.
fn extract_address(s: &str) -> &str {
    let s = s.trim();
    // Prefer angle-bracket address
    if let Some(lt) = s.find('<')
        && let Some(gt) = s[lt..].find('>')
    {
        return &s[lt + 1..lt + gt];
    }
    // Skip RFC 822 comments and quoted strings, return first word
    let mut i = 0;
    let b = s.as_bytes();
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'(' => {
                let mut depth = 1;
                i += 1;
                while i < b.len() && depth > 0 {
                    match b[i] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        b'\\' => i += 1,
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => {
                let start = i;
                while i < b.len()
                    && !matches!(b[i], b' ' | b'\t' | b'\n' | b'\r')
                {
                    if b[i] == b'"' {
                        i += 1;
                        while i < b.len() && b[i] != b'"' {
                            if b[i] == b'\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                        if i < b.len() {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
                return &s[start..i];
            }
        }
    }
    ""
}

/// Address quality penalty (procmail formail.c:282-289).
/// Higher penalty = worse address. Multiplier = SEST.len() + 1 = 12.
fn addr_penalty(addr: &str) -> i32 {
    const MUL: i32 = (SEST.len() + 1) as i32;
    if !addr.contains('@') && !addr.contains('!') && !addr.contains('/') {
        MUL * 4
    } else if addr.contains(".UUCP") {
        MUL * 3
    } else if addr.contains('@') && !addr.contains('.') {
        MUL * 2
    } else if addr.contains('!') {
        MUL
    } else {
        0
    }
}

/// Extract address from a From_ line with UUCP bang-path reconstruction.
fn from_line_addr(raw: &str) -> (String, i32) {
    let mut penalty = 0;
    if raw.contains('\n') {
        penalty = 2;
    }
    // Skip to last UUCP ">From" line
    let mut addr_line = raw;
    for part in raw.split('\n') {
        if part.starts_with("From ") || part.starts_with(">From ") {
            addr_line = part;
        }
    }
    // Extract first word after "From " or ">From "
    let rest = addr_line
        .strip_prefix(">From ")
        .or_else(|| addr_line.strip_prefix("From "))
        .unwrap_or(addr_line);
    let user = rest.split_whitespace().next().unwrap_or("");
    if user.is_empty() {
        return (String::new(), penalty);
    }
    if user.contains('@') {
        return (user.to_string(), penalty);
    }
    // Reconstruct bang-path from "remote from"/"forwarded by" info
    let mut path = String::new();
    let mut search = raw;
    loop {
        let rf = search.find(" remote from ");
        let fb = search.find(" forwarded by ");
        let (pos, skip) = match (rf, fb) {
            (Some(a), Some(b)) if b < a => (b, " forwarded by ".len()),
            (Some(a), _) => (a, " remote from ".len()),
            (None, Some(b)) => (b, " forwarded by ".len()),
            (None, None) => break,
        };
        let hop = &search[pos + skip..];
        for ch in hop.bytes() {
            match ch {
                0 | b'\n' => break,
                _ => path.push(ch as char),
            }
        }
        path.push('!');
        search = &search[pos + skip..];
        // advance past the word
        if let Some(nl) = search.find('\n') {
            search = &search[nl..];
        } else {
            break;
        }
    }
    path.push_str(user);
    (path, penalty)
}

/// Unified sender detection matching procmail's getsender().
fn get_sender(fields: &FieldList, mode: SenderMode) -> Option<String> {
    let first = fields.iter().next();
    let mut best: Option<String> = None;
    let mut best_w: i32 = i32::MIN;

    for f in fields.iter() {
        let (idx, is_from) = if f.is_from_line() {
            (SEST.len() - 1, true)
        } else {
            let mut found = None;
            for (i, sf) in SEST.iter().enumerate() {
                if !sf.name.contains(&b' ') && f.name_matches(sf.name) {
                    found = Some(i);
                    break;
                }
            }
            let Some(i) = found else { continue };
            (i, false)
        };
        // From_ only counts if it's the first field
        if is_from && !first.is_some_and(|ff| std::ptr::eq(ff, f)) {
            continue;
        }
        let mut w = match mode {
            SenderMode::Envelope => idx as i32,
            SenderMode::Reply => SEST[idx].reply_weight,
            SenderMode::ResentReply => SEST[idx].resent_reply_weight,
        };
        let addr = if is_from {
            let Ok(s) = str::from_utf8(f.value()) else {
                continue;
            };
            let (a, pen) = from_line_addr(s);
            w -= pen;
            a
        } else {
            let Ok(s) = str::from_utf8(f.value()) else {
                continue;
            };
            extract_address(s).to_string()
        };
        if addr.is_empty() {
            // Empty Return-Path → address is "<>", override weight
            if SEST[idx].name == b"Return-Path:" {
                let ow = SEST.len() as i32 + 1;
                if ow > best_w {
                    best_w = ow;
                    best = Some("<>".to_string());
                }
            }
            continue;
        }
        w -= addr_penalty(&addr);
        if w > best_w {
            best_w = w;
            best = Some(addr);
        }
    }
    best
}

fn generate_message_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let pid = std::process::id();
    let host = nix::unistd::gethostname()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "localhost".to_string());

    format!("<{}.{}@{}>", ts, pid, host)
}

fn generate_from_line(fields: &FieldList) -> Vec<u8> {
    let sender =
        get_sender(fields, SenderMode::Envelope).unwrap_or("UNKNOWN".into());
    let timestamp = chrono::Local::now().format("%a %b %e %H:%M:%S %Y");
    format!("From {} {}\n", sender, timestamp).into_bytes()
}

/// Generate auto-reply headers from original message.
fn generate_reply(args: &Args, orig: &FieldList) -> FieldList {
    let mut reply = FieldList::new();

    let resent = args
        .add_if_not
        .iter()
        .any(|h| h.eq_ignore_ascii_case("Resent-"));
    let mode = if args.trust && resent {
        SenderMode::ResentReply
    } else if args.trust {
        SenderMode::Reply
    } else {
        SenderMode::Envelope
    };
    let addr = get_sender(orig, mode).unwrap_or_else(|| "UNKNOWN".into());
    reply.push(Field::from_parts(b"To:", addr.as_bytes()));

    // Subject with Re: prefix (procmail always adds Re:, even if already
    // present)
    if let Some(subj) = orig.find(b"Subject") {
        let s = String::from_utf8_lossy(subj.value());
        let new_subj = format!("Re:{}", s);
        reply.push(Field::from_parts(b"Subject:", new_subj.as_bytes()));
    }

    // References: combine old References + old Message-ID
    let mut refs = String::new();
    if let Some(f) = orig.find(b"References") {
        let s = String::from_utf8_lossy(f.value());
        refs.push_str(s.trim());
    }
    if let Some(f) = orig.find(b"Message-ID") {
        let s = String::from_utf8_lossy(f.value());
        if !refs.is_empty() {
            refs.push(' ');
        }
        refs.push_str(s.trim());
    }
    if !refs.is_empty() {
        reply.push(Field::from_parts(b"References:", refs.as_bytes()));
    }

    // In-Reply-To: original Message-ID
    if let Some(f) = orig.find(b"Message-ID") {
        reply.push(Field::from_parts(b"In-Reply-To:", f.value()));
    }

    // Preserve X-Loop: fields
    for f in orig.iter() {
        if f.name_matches(b"X-Loop") {
            reply.push(f.clone());
        }
    }

    // Preserve fields specified with -i
    for h in &args.rename_insert {
        let (name, _) = parse_header_arg(h);
        for f in orig.iter() {
            if f.name_matches(name.as_bytes()) {
                reply.push(f.clone());
            }
        }
    }

    reply
}

fn process_headers(args: &Args, fields: &mut FieldList) -> io::Result<()> {
    for h in &args.delete_insert {
        let (name, value) = parse_header_arg(h);
        fields.remove_all(name.as_bytes());
        if !value.is_empty() {
            fields.push(Field::from_parts(name.as_bytes(), value.as_bytes()));
        }
    }

    for h in &args.rename_insert {
        let (name, value) = parse_header_arg(h);
        fields.prepend_old(name.as_bytes());
        if !value.is_empty() {
            fields.push(Field::from_parts(name.as_bytes(), value.as_bytes()));
        }
    }

    if !args.rename.len().is_multiple_of(2) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "-R requires pairs of old and new field names",
        ));
    }
    for pair in args.rename.chunks(2) {
        fields.rename_all(pair[0].as_bytes(), pair[1].as_bytes());
    }

    for h in &args.first_uniq {
        fields.keep_first(h.as_bytes());
    }

    for h in &args.last_uniq {
        fields.keep_last(h.as_bytes());
    }

    for h in &args.add_if_not {
        // "Resent-" is a flag, not a real header (sets resent-reply mode)
        if h.eq_ignore_ascii_case("Resent-") {
            continue;
        }
        let (name, value) = parse_header_arg(h);
        if fields.find(name.as_bytes()).is_none() {
            // Message-ID with no value generates unique ID
            if (name.eq_ignore_ascii_case("Message-ID")
                || name.eq_ignore_ascii_case("Resent-Message-ID"))
                && value.is_empty()
            {
                let id = generate_message_id();
                fields.push(Field::from_parts(name.as_bytes(), id.as_bytes()));
            } else {
                fields
                    .push(Field::from_parts(name.as_bytes(), value.as_bytes()));
            }
        }
    }

    for h in &args.add_always {
        let (name, value) = parse_header_arg(h);
        fields.push(Field::from_parts(name.as_bytes(), value.as_bytes()));
    }

    if args.concatenate {
        for f in fields.iter_mut() {
            f.concatenate();
        }
    }

    if args.zap {
        fields.zap_whitespace();
    }

    Ok(())
}

fn output_extracted(
    args: &Args, fields: &FieldList, out: &mut impl Write,
) -> io::Result<()> {
    for pattern in &args.extract {
        for f in fields.iter() {
            if f.name_matches(pattern.as_bytes()) {
                let mut val = f.value();
                if args.zap {
                    val = val.trim_ascii();
                }
                out.write_all(val)?;
                out.write_all(b"\n")?;
            }
        }
    }

    for pattern in &args.extract_keep {
        for f in fields.iter() {
            if f.name_matches(pattern.as_bytes()) {
                out.write_all(&f.text)?;
            }
        }
    }

    Ok(())
}

fn output_body<R, W>(
    initial: &[u8], reader: &mut R, out: &mut W, quote: Quote, prefix: &str,
) -> io::Result<bool>
where
    R: BufRead,
    W: Write,
{
    let mut wrote = !initial.is_empty();
    for line in initial.split_inclusive(|&b| b == b'\n') {
        if quote.prefix(line) {
            out.write_all(prefix.as_bytes())?;
        }
        out.write_all(line)?;
    }

    let mut line = Vec::with_capacity(1024);
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        wrote = true;
        if quote.prefix(&line) {
            out.write_all(prefix.as_bytes())?;
        }
        out.write_all(&line)?;
    }

    Ok(wrote)
}

fn output_log_summary(
    folder: &str, fields: &FieldList, len: usize, out: &mut impl Write,
) -> io::Result<()> {
    const TAB: usize = 8;
    const MAX_SUBJ: usize = 78;

    if let Some(f) = fields.iter().next()
        && f.is_from_line()
    {
        out.write_all(&f.text)?;
    }

    if let Some(subj) = fields.find(b"Subject") {
        let s = String::from_utf8_lossy(subj.value());
        let s = s.trim().replace('\t', " ");
        let s = if s.len() > MAX_SUBJ {
            &s[..MAX_SUBJ]
        } else {
            &s
        };
        out.write_all(b" Subject: ")?;
        out.write_all(s.as_bytes())?;
        out.write_all(b"\n")?;
    }

    let prefix = "  Folder: ";
    out.write_all(prefix.as_bytes())?;
    out.write_all(folder.as_bytes())?;

    // Right-align size to column 79
    let num = format!("{len}");
    let mut col = prefix.len() + folder.len();
    let target = 79 - num.len();
    while col < target {
        let next = (col / TAB + 1) * TAB;
        if next <= target {
            out.write_all(b"\t")?;
            col = next;
        } else {
            break;
        }
    }
    while col < target {
        out.write_all(b" ")?;
        col += 1;
    }

    writeln!(out, "{num}")?;
    Ok(())
}

/// Check if message is a duplicate using ID cache.
/// Returns true if duplicate found.
///
/// Cache format: null-terminated strings in a circular buffer.
/// When adding a new entry would exceed maxlen, wrap to start.
/// An empty entry (just \0) marks the end of valid data.
fn check_duplicate(
    args: &Args, fields: &FieldList, cache: &str, maxlen: usize,
) -> io::Result<bool> {
    let key = if args.reply {
        let mode = if args.trust {
            SenderMode::Reply
        } else {
            SenderMode::Envelope
        };
        get_sender(fields, mode).unwrap_or_default()
    } else {
        fields
            .find(b"Message-ID")
            .and_then(|f| std::str::from_utf8(f.value()).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };

    let key = key.trim_start();
    if key.is_empty() {
        return Ok(false);
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache)?;

    // Lock file to prevent concurrent access corruption
    let mut file = Flock::lock(file, FlockArg::LockExclusive)
        .map_err(|(_, e)| io::Error::other(e))?;

    let mut contents = vec![0u8; maxlen];
    let n = file.read(&mut contents)?;
    contents.truncate(n);

    let mut dup = false;
    let mut insert: Option<usize> = None;

    let mut pos = 0;
    while pos < contents.len() {
        let start = pos;
        while pos < contents.len() && contents[pos] != 0 {
            pos += 1;
        }
        let entry = &contents[start..pos];

        if entry.is_empty() {
            if insert.is_none() {
                insert = Some(start);
            }
        } else if entry == key.as_bytes() {
            dup = true;
            break;
        }

        if pos < contents.len() {
            pos += 1;
        }
    }

    if !dup {
        let offset = if let Some(off) = insert {
            off
        } else if n >= maxlen {
            0
        } else {
            n
        };

        let needed = key.len() + 2;
        let offset = if offset + needed > maxlen { 0 } else { offset };

        file.seek(SeekFrom::Start(offset as u64))?;
        file.write_all(key.as_bytes())?;
        file.write_all(b"\0\0")?;
        file.set_len((offset + needed) as u64)?;
    }

    Ok(dup)
}

fn output_message(
    pool: &mut Option<ProcessPool>, cmd: &[String], msg: &[u8], num: usize,
    base: i64, width: usize,
) -> io::Result<()> {
    if cmd.is_empty() {
        io::stdout().write_all(msg)?;
        // mbox format: blank line after message
        io::stdout().write_all(b"\n")?;
    } else {
        let fileno =
            format!("{:0width$}", base + num as i64, width = width.max(1));
        let mut child = Command::new(&cmd[0])
            .args(&cmd[1..])
            .env("FILENO", &fileno)
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(msg)?;
        }

        match pool {
            Some(p) => p.spawn(child)?,
            None => {
                child.wait()?;
            }
        }
    }
    Ok(())
}

fn start_pending_header(state: &mut SplitState, line: &[u8]) {
    state.pending.extend_from_slice(line);
}

fn accumulate_header(cfg: &SplitConfig, state: &mut SplitState, line: &[u8]) {
    if line == b"\n" || line == b"\r\n" {
        if state.fields >= cfg.minfields {
            state.in_msg = true;
            if !cfg.digest {
                state.content_remaining = parse_content_length(&state.pending);
            }
            state.msg.extend_from_slice(&state.pending);
            state.msg.extend_from_slice(line);
        }
        state.pending.clear();
        state.fields = 0;
    } else if is_header_field(line) {
        state.pending.extend_from_slice(line);
        state.fields += 1;
    } else if line.starts_with(b" ") || line.starts_with(b"\t") {
        state.pending.extend_from_slice(line);
    } else {
        state.pending.clear();
        state.fields = 0;
    }
}

fn process_content(cfg: &SplitConfig, state: &mut SplitState, line: &[u8]) {
    if !state.pending.is_empty() {
        accumulate_header(cfg, state, line);
    } else if state.in_msg {
        state.msg.extend_from_slice(line);
    }
}

fn process_boundary(
    state: &mut SplitState, line: &[u8],
) -> io::Result<Option<Vec<u8>>> {
    let prev = if state.in_msg && !state.msg.is_empty() {
        let mut msg = mem::take(&mut state.msg);
        if msg.ends_with(b"\n\n") {
            msg.pop();
        }
        Some(msg)
    } else {
        None
    };

    state.msg.clear();
    state.pending.clear();
    start_pending_header(state, line);
    state.fields = if is_header_field(line) { 1 } else { 0 };
    state.in_msg = false;

    Ok(prev)
}

fn process_line(
    cfg: &SplitConfig, state: &mut SplitState, line: &[u8],
) -> io::Result<Option<Vec<u8>>> {
    if cfg.is_boundary(line, state.last_blank) {
        return process_boundary(state, line);
    }

    process_content(cfg, state, line);
    Ok(None)
}

fn emit_message(
    cfg: &SplitConfig, pool: &mut Option<ProcessPool>, state: &mut SplitState,
    msg: &[u8],
) -> io::Result<bool> {
    state.msg_num += 1;
    if state.msg_num <= cfg.skip {
        return Ok(true);
    }
    if let Some(t) = cfg.total
        && state.count >= t
    {
        return Ok(false);
    }
    output_message(pool, cfg.cmd, msg, state.count, cfg.base, cfg.width)?;
    state.count += 1;
    Ok(true)
}

fn flush_final(
    cfg: &SplitConfig, pool: &mut Option<ProcessPool>, state: &SplitState,
) -> io::Result<()> {
    if state.in_msg && !state.msg.is_empty() {
        let num = state.msg_num + 1;
        if num > cfg.skip && cfg.total.is_none_or(|t| state.count < t) {
            output_message(
                pool,
                cfg.cmd,
                &state.msg,
                state.count,
                cfg.base,
                cfg.width,
            )?;
        }
    }
    Ok(())
}

fn run_split_with_env(args: Args, env: &Environment) -> io::Result<i32> {
    let stdin = io::stdin().lock();
    let mut reader = io::BufReader::new(stdin);

    let fileno = env.get("FILENO");
    let cfg = SplitConfig {
        cmd: args.split.as_ref().unwrap(),
        skip: args.skip.unwrap_or(0),
        total: args.total,
        minfields: args.minfields.unwrap_or(2),
        every: args.every,
        digest: args.digest,
        base: fileno.and_then(|s| s.parse().ok()).unwrap_or(0),
        width: fileno.map(|s| s.len()).unwrap_or(0),
    };
    let mut pool = args.nowait.map(ProcessPool::new);
    let mut state = SplitState::new();

    let mut line = Vec::new();
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            flush_final(&cfg, &mut pool, &state)?;
            break;
        }

        if let Some(msg) = process_line(&cfg, &mut state, &line)?
            && !emit_message(&cfg, &mut pool, &mut state, &msg)?
        {
            break;
        }

        if state.content_remaining > 0 {
            let want = state.content_remaining;
            let got = (&mut reader)
                .take(want as u64)
                .read_to_end(&mut state.msg)?;
            if got < want {
                eprintln!(
                    "formail: Content-Length: field exceeds actual length by {} bytes",
                    want - got,
                );
            }
            state.content_remaining = 0;
            state.last_blank = state.msg.ends_with(b"\n\n");
            continue;
        }

        state.last_blank = line == b"\n" || line == b"\r\n";
    }

    if let Some(mut p) = pool {
        p.wait_all()?;
    }

    Ok(util::EX_OK as i32)
}

/// Run in split mode - split mailbox/digest into messages.
fn run_split(args: Args) -> io::Result<i32> {
    run_split_with_env(args, &Environment::from_process())
}

fn run(args: Args) -> io::Result<i32> {
    if args.split.is_some() {
        return run_split(args);
    }

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let (mut fields, body) = read_header(&mut stdin)?;

    // 0 = duplicate, 1 = unique per procmail convention
    if let Some(ref dup_args) = args.duplicate
        && dup_args.len() >= 2
    {
        let maxlen: usize = dup_args[0].parse().unwrap_or(8192);
        let path = &dup_args[1];
        let dup = check_duplicate(&args, &fields, path, maxlen)?;
        let code = if dup { util::EX_OK } else { 1 };
        return Ok(code as i32);
    }

    if args.reply {
        let reply = generate_reply(&args, &fields);
        reply.write_to(&mut stdout)?;
        stdout.write_all(b"\n")?;

        if args.keep_body {
            let prefix = args.prefix.as_deref().unwrap_or(">");
            let quote = if args.no_escape {
                Quote::None
            } else {
                Quote::All
            };
            let had_body =
                output_body(&body, &mut stdin, &mut stdout, quote, prefix)?;
            if had_body {
                // mbox format: blank line after message
                stdout.write_all(b"\n")?;
            }
        }
        return Ok(util::EX_OK as i32);
    }

    process_headers(&args, &mut fields)?;

    if let Some(ref folder) = args.log {
        let mut total: usize = fields.iter().map(|f| f.len()).sum();
        total += 1 + body.len();
        let mut rest = Vec::new();
        stdin.read_to_end(&mut rest)?;
        total += rest.len();
        // mbox format adds trailing blank line
        total += 1;

        output_log_summary(folder, &fields, total, &mut stdout)?;
        return Ok(util::EX_OK as i32);
    }

    let need_from = !args.force
        && !fields.is_empty()
        && !fields.iter().next().is_some_and(|f| f.is_from_line());

    if !args.extract.is_empty() || !args.extract_keep.is_empty() {
        output_extracted(&args, &fields, &mut stdout)?;
    } else {
        if need_from {
            let from = generate_from_line(&fields);
            stdout.write_all(&from)?;
        }

        fields.write_to(&mut stdout)?;
        stdout.write_all(b"\n")?;

        if args.keep_body
            || (args.extract.is_empty() && args.extract_keep.is_empty())
        {
            let prefix = args.prefix.as_deref().unwrap_or(">");
            let quote = if args.no_escape {
                Quote::None
            } else {
                Quote::From
            };
            let had_body =
                output_body(&body, &mut stdin, &mut stdout, quote, prefix)?;
            if had_body {
                // mbox format: blank line after message
                stdout.write_all(b"\n")?;
            }
        }
    }

    Ok(util::EX_OK as i32)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (skip, total, rest) = parse_skip_total(&args);

    let args = match Args::try_parse_from(rest) {
        Ok(mut a) => {
            if skip.is_some() {
                a.skip = skip;
            }
            if total.is_some() {
                a.total = total;
            }
            a
        }
        Err(e) => {
            let _ = e.print();
            let code = if e.use_stderr() { util::EX_USAGE } else { 0 };
            return ExitCode::from(code);
        }
    };

    match run(args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("formail: {e}");
            ExitCode::from(util::EX_UNAVAILABLE)
        }
    }
}
