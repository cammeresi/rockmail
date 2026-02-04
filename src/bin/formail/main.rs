//! formail - mail (re)formatter
//!
//! Converts mail to mailbox format, performs From_ escaping, generates
//! auto-reply headers, header manipulation, and mailbox/digest splitting.

use std::fs::OpenOptions;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use nix::fcntl::{Flock, FlockArg};

use corpmail::formail::{Field, FieldList, read_header};
use corpmail::util;
use corpmail::variables::{Env, RealEnv};

#[cfg(test)]
mod tests;

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

    /// BABYL rmail file format
    #[arg(short = 'B')]
    babyl: bool,

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
    const OFFSET: usize = 64;
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
        out.write_all(b" ")?;
        out.write_all(s.as_bytes())?;
        out.write_all(b"\n")?;
    }

    let prefix = "  Folder: ";
    out.write_all(prefix.as_bytes())?;
    out.write_all(folder.as_bytes())?;

    let mut col = prefix.len() + folder.len();
    col -= col % TAB;
    while col < OFFSET {
        out.write_all(b"\t")?;
        col += TAB;
    }

    writeln!(out, "{len}")?;
    Ok(())
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

fn generate_from_line(fields: &FieldList) -> Vec<u8> {
    let sender = find_sender(fields).unwrap_or("UNKNOWN");
    let timestamp = chrono::Local::now().format("%a %b %e %H:%M:%S %Y");
    format!("From {} {}\n", sender, timestamp).into_bytes()
}

fn find_sender(fields: &FieldList) -> Option<&str> {
    // Priority order for sender detection
    const SENDER_FIELDS: &[&str] = &[
        "Return-Path",
        "From",
        "Sender",
        "Reply-To",
        "Resent-From",
        "Resent-Sender",
    ];

    for name in SENDER_FIELDS {
        if let Some(f) = fields.find(name.as_bytes()) {
            let val = f.value();
            if let Ok(s) = std::str::from_utf8(val) {
                let addr = extract_address(s.trim());
                if !addr.is_empty() {
                    return Some(addr);
                }
            }
        }
    }
    None
}

fn extract_address(s: &str) -> &str {
    if let Some(start) = s.rfind('<')
        && let Some(end) = s[start..].find('>')
    {
        return &s[start + 1..start + end];
    }
    s.split_whitespace().next().unwrap_or("")
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

/// Generate auto-reply headers from original message.
fn generate_reply(args: &Args, orig: &FieldList) -> FieldList {
    let mut reply = FieldList::new();

    // Determine reply address
    let addr =
        find_reply_address(args, orig).unwrap_or_else(|| "UNKNOWN".to_string());
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

// BABYL format separators
const BABYL_SEP1: u8 = 0x1F; // control-_
const BABYL_SEP2: u8 = 0x0C; // form feed

/// Run in split mode - split mailbox/digest into messages.
fn run_split(args: Args) -> io::Result<i32> {
    run_split_with_env(args, &RealEnv)
}

fn run_split_with_env<E>(args: Args, env: &E) -> io::Result<i32>
where
    E: Env,
{
    let stdin = io::stdin().lock();
    let mut reader = io::BufReader::new(stdin);
    let cmd = args.split.as_ref().unwrap();

    let skip = args.skip.unwrap_or(0);
    let total = args.total;
    let minfields = args.minfields.unwrap_or(2);

    // -B (berkeley/BABYL) implies every mode
    let every = args.every || args.babyl;
    let digest = args.digest || args.babyl;

    let base: i64 = env.get("FILENO").and_then(|s| s.parse().ok()).unwrap_or(0);
    let width = env.get("FILENO").map(|s| s.len()).unwrap_or(0);
    let mut pool = args.nowait.map(ProcessPool::new);

    if args.babyl {
        skip_babyl_leader(&mut reader)?;
    }

    let mut msg_num = 0usize;
    let mut count = 0usize;
    let mut line = Vec::new();
    let mut msg = Vec::new();
    let mut pending_header = Vec::new();
    let mut header_fields = 0;
    let mut last_blank = true;
    let mut in_msg = false;
    let mut babyl_start = false;

    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            if in_msg && !msg.is_empty() {
                msg_num += 1;
                if msg_num > skip && total.is_none_or(|t| count < t) {
                    output_message(&mut pool, cmd, &msg, count, base, width)?;
                }
            }
            break;
        }

        // Check for BABYL separator
        if args.babyl && line.starts_with(&[BABYL_SEP1]) {
            babyl_start = true;
            // Output previous message if any
            if in_msg && !msg.is_empty() {
                msg_num += 1;
                if msg_num > skip {
                    if let Some(t) = total
                        && count >= t
                    {
                        break;
                    }
                    output_message(&mut pool, cmd, &msg, count, base, width)?;
                    count += 1;
                }
            }
            // Skip until end of BABYL pseudo header
            skip_babyl_header(&mut reader)?;
            msg.clear();
            pending_header.clear();
            header_fields = 0;
            in_msg = false;
            last_blank = true;
            continue;
        }

        // In BABYL mode, don't split on regular boundaries
        if args.babyl && !babyl_start {
            if in_msg {
                msg.extend_from_slice(&line);
            }
            last_blank = line == b"\n" || line == b"\r\n";
            continue;
        }

        // Check for message boundary
        let is_boundary = if digest || every {
            is_header_field(&line) && (every || last_blank)
        } else {
            line.starts_with(b"From ") && last_blank
        };

        if is_boundary {
            // Output previous message if any
            if in_msg && !msg.is_empty() {
                // Strip trailing blank line (before this boundary)
                if msg.ends_with(b"\n\n") {
                    msg.pop();
                }
                msg_num += 1;
                if msg_num > skip {
                    if let Some(t) = total
                        && count >= t
                    {
                        break;
                    }
                    output_message(&mut pool, cmd, &msg, count, base, width)?;
                    count += 1;
                }
            }

            // Start new message
            msg.clear();
            pending_header.clear();
            // Convert Mail-from: to From (BABYL format)
            if args.babyl && line.len() > 10 {
                let lower: Vec<u8> =
                    line[..10].iter().map(|b| b.to_ascii_lowercase()).collect();
                if &lower == b"mail-from:" {
                    pending_header.extend_from_slice(b"From ");
                    let rest = &line[10..];
                    let rest = rest.strip_prefix(b" ").unwrap_or(rest);
                    pending_header.extend_from_slice(rest);
                } else {
                    pending_header.extend_from_slice(&line);
                }
            } else {
                pending_header.extend_from_slice(&line);
            }
            header_fields = if is_header_field(&line) { 1 } else { 0 };
            in_msg = false;
            babyl_start = false;
        } else if !pending_header.is_empty() {
            // Accumulating header
            if line == b"\n" || line == b"\r\n" {
                // End of header
                if header_fields >= minfields {
                    in_msg = true;
                    msg.extend_from_slice(&pending_header);
                    msg.extend_from_slice(&line);
                }
                pending_header.clear();
                header_fields = 0;
            } else if is_header_field(&line) {
                pending_header.extend_from_slice(&line);
                header_fields += 1;
            } else if line.starts_with(b" ") || line.starts_with(b"\t") {
                // Continuation
                pending_header.extend_from_slice(&line);
            } else {
                // Not a valid header
                pending_header.clear();
                header_fields = 0;
            }
        } else if in_msg {
            msg.extend_from_slice(&line);
        }

        last_blank = line == b"\n" || line == b"\r\n";
    }

    if let Some(mut p) = pool {
        p.wait_all()?;
    }

    Ok(util::EX_OK as i32)
}

/// Skip the BABYL leader (everything until the first separator).
fn skip_babyl_leader<R: BufRead>(reader: &mut R) -> io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        if line.starts_with(&[BABYL_SEP1, BABYL_SEP2]) {
            break;
        }
    }
    // Skip the line after the separator
    line.clear();
    reader.read_until(b'\n', &mut line)?;
    Ok(())
}

/// Skip BABYL pseudo header (until blank line).
fn skip_babyl_header<R: BufRead>(reader: &mut R) -> io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 || line == b"\n" || line == b"\r\n" {
            break;
        }
    }
    Ok(())
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
        find_reply_address(args, fields).unwrap_or_default()
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

fn find_reply_address(args: &Args, fields: &FieldList) -> Option<String> {
    // -t uses header sender (Reply-To, From), else envelope (Return-Path,
    // From_)
    if args.trust {
        const FIELDS: &[&str] = &["Reply-To", "From", "Sender"];
        for name in FIELDS {
            if let Some(f) = fields.find(name.as_bytes())
                && let Ok(s) = std::str::from_utf8(f.value())
            {
                let addr = extract_address(s.trim());
                if !addr.is_empty() {
                    return Some(addr.to_string());
                }
            }
        }
    } else {
        if let Some(f) = fields.find(b"Return-Path")
            && let Ok(s) = std::str::from_utf8(f.value())
        {
            let addr = extract_address(s.trim());
            if !addr.is_empty() && addr != "<>" {
                return Some(addr.to_string());
            }
        }
        if let Some(f) = fields.iter().next()
            && f.is_from_line()
            && let Ok(s) = std::str::from_utf8(f.value())
        {
            let addr = s.split_whitespace().next().unwrap_or("");
            if !addr.is_empty() {
                return Some(addr.to_string());
            }
        }
        if let Some(f) = fields.find(b"From")
            && let Ok(s) = std::str::from_utf8(f.value())
        {
            let addr = extract_address(s.trim());
            if !addr.is_empty() {
                return Some(addr.to_string());
            }
        }
    }
    None
}
