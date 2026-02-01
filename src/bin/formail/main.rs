//! formail - mail (re)formatter
//!
//! Converts mail to mailbox format, performs From_ escaping, generates
//! auto-reply headers, header manipulation, and mailbox/digest splitting.

use std::io::{self, BufRead, Read, Write};
use std::process::{Command, ExitCode, Stdio};

use clap::Parser;

use corpmail::formail::{Field, FieldList, read_header};
use corpmail::util;
use corpmail::variables::{Env, RealEnv};

#[cfg(test)]
mod tests;

/// formail - mail (re)formatter
#[derive(Parser, Debug, Default)]
#[command(name = "formail", version, about)]
#[command(
    override_usage = "formail [+skip] [-total] [options] [-s [command [arg ...]]]"
)]
struct Args {
    /// Skip the first N messages while splitting
    #[arg(short = '+', value_name = "skip", hide = true)]
    skip: Option<usize>,

    /// Output at most N messages while splitting (use -N syntax)
    #[arg(value_name = "total", hide = true)]
    total: Option<usize>,

    /// Don't escape bogus From_ lines
    #[arg(short = 'b')]
    no_escape: bool,

    /// Berkeley format (ignore Content-Length)
    #[arg(short = 'B')]
    berkeley: bool,

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

    /// Don't wait for programs (parallel split), optional max procs
    #[arg(short = 'n', value_name = "maxprocs")]
    nowait: Option<Option<usize>>,

    /// Quotation prefix for From_ escaping (default ">")
    #[arg(short = 'p', value_name = "prefix")]
    prefix: Option<String>,

    /// Be quiet about errors (default on, use -q- to disable)
    #[arg(short = 'q', action = clap::ArgAction::Count)]
    quiet: u8,

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
    /// Add header if not present (use "Name:" for just name, generates Message-ID)
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
    #[arg(short = 'R', value_names = ["old", "new"], action = clap::ArgAction::Append)]
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
    // Handle special +N and -N arguments before clap parsing
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
            eprintln!("{e}");
            return ExitCode::from(util::EX_USAGE);
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
        if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 1 {
            let c = arg.chars().nth(1).unwrap();
            if c.is_ascii_digit()
                && let Ok(v) = arg[1..].parse::<usize>()
            {
                total = Some(v);
                i += 1;
                continue;
            }
        }
        break;
    }

    rest.extend(args[i..].iter().cloned());
    (skip, total, rest)
}

fn run(args: Args) -> io::Result<i32> {
    // Split mode handles input differently
    if args.split.is_some() {
        return run_split(args);
    }

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    // Read header from stdin
    let (mut fields, body) = read_header(&mut stdin)?;

    // Duplicate detection
    if let Some(ref dup_args) = args.duplicate
        && dup_args.len() >= 2
    {
        let maxlen: usize = dup_args[0].parse().unwrap_or(8192);
        let cache_path = &dup_args[1];
        let is_dup = check_duplicate(&args, &fields, cache_path, maxlen)?;
        if args.split.is_none() {
            // Not splitting - exit with status based on duplicate
            return Ok(if is_dup { 0 } else { 1 });
        }
        // If splitting and duplicate, suppress output (handled elsewhere)
    }

    // Reply mode generates auto-reply headers
    if args.reply {
        let reply = generate_reply(&args, &fields);
        reply.write_to(&mut stdout)?;
        stdout.write_all(b"\n")?;

        if args.keep_body {
            let prefix = args.prefix.as_deref().unwrap_or(">");
            output_body(
                &body,
                &mut stdin,
                &mut stdout,
                !args.no_escape,
                prefix,
            )?;
        }
        return Ok(util::EX_OK as i32);
    }

    // Apply header operations
    process_headers(&args, &mut fields)?;

    // Log summary mode - output summary instead of message
    if let Some(ref folder) = args.log {
        // Calculate total message length
        let mut total: usize = fields.iter().map(|f| f.len()).sum();
        total += 1; // blank line after header
        total += body.len();
        // Read rest of stdin to get full length
        let mut rest = Vec::new();
        stdin.read_to_end(&mut rest)?;
        total += rest.len();

        output_log_summary(folder, &fields, total, &mut stdout)?;
        return Ok(util::EX_OK as i32);
    }

    // Determine if we need to add From_ line
    let need_from = !args.force
        && !fields.is_empty()
        && !fields.iter().next().is_some_and(|f| f.is_from_line());

    // Output
    if !args.extract.is_empty() || !args.extract_keep.is_empty() {
        // Extract mode - just output matching fields
        output_extracted(&args, &fields, &mut stdout)?;
    } else {
        // Normal mode - output headers
        if need_from {
            let from = generate_from_line(&fields);
            stdout.write_all(&from)?;
        }

        fields.write_to(&mut stdout)?;
        stdout.write_all(b"\n")?;

        // Output body unless extracting without -k
        if args.keep_body
            || (args.extract.is_empty() && args.extract_keep.is_empty())
        {
            let prefix = args.prefix.as_deref().unwrap_or(">");
            output_body(
                &body,
                &mut stdin,
                &mut stdout,
                !args.no_escape,
                prefix,
            )?;
        }
    }

    Ok(util::EX_OK as i32)
}

fn process_headers(args: &Args, fields: &mut FieldList) -> io::Result<()> {
    // -I: delete matching fields, then add new
    for h in &args.delete_insert {
        let (name, value) = parse_header_arg(h);
        fields.remove_all(name.as_bytes());
        if !value.is_empty() {
            fields.push(Field::from_parts(name.as_bytes(), value.as_bytes()));
        }
    }

    // -i: rename matching fields to Old-*, then add new
    for h in &args.rename_insert {
        let (name, value) = parse_header_arg(h);
        fields.prepend_old(name.as_bytes());
        if !value.is_empty() {
            fields.push(Field::from_parts(name.as_bytes(), value.as_bytes()));
        }
    }

    // -R: rename fields
    let renames: Vec<_> = args.rename.chunks(2).collect();
    for pair in renames {
        if pair.len() == 2 {
            fields.rename_all(pair[0].as_bytes(), pair[1].as_bytes());
        }
    }

    // -u: keep first occurrence
    for h in &args.first_uniq {
        fields.keep_first(h.as_bytes());
    }

    // -U: keep last occurrence
    for h in &args.last_uniq {
        fields.keep_last(h.as_bytes());
    }

    // -a: add if not present
    for h in &args.add_if_not {
        let (name, value) = parse_header_arg(h);
        if fields.find(name.as_bytes()).is_none() {
            // Special case: Message-ID: with no value generates unique ID
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

    // -A: add always
    for h in &args.add_always {
        let (name, value) = parse_header_arg(h);
        fields.push(Field::from_parts(name.as_bytes(), value.as_bytes()));
    }

    // -c: concatenate continuation lines
    if args.concatenate {
        for f in fields.iter_mut() {
            f.concatenate();
        }
    }

    // -z: zap whitespace
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

fn output_body(
    initial: &[u8], reader: &mut impl Read, out: &mut impl Write, escape: bool,
    prefix: &str,
) -> io::Result<()> {
    let mut buf = initial.to_vec();
    reader.read_to_end(&mut buf)?;

    if escape {
        for line in buf.split_inclusive(|&b| b == b'\n') {
            if line.starts_with(b"From ") {
                out.write_all(prefix.as_bytes())?;
            }
            out.write_all(line)?;
        }
    } else {
        out.write_all(&buf)?;
    }

    Ok(())
}

/// Output log summary in procmail format.
fn output_log_summary(
    folder: &str, fields: &FieldList, total_len: usize, out: &mut impl Write,
) -> io::Result<()> {
    const TAB_WIDTH: usize = 8;
    const LEN_OFFSET: usize = 64; // 8 * 8 tab stops
    const MAX_SUBJECT: usize = 78;

    // Output From_ line if present
    if let Some(f) = fields.iter().next()
        && f.is_from_line()
    {
        out.write_all(&f.text)?;
    }

    // Output Subject (truncated)
    if let Some(subj) = fields.find(b"Subject")
        && let Ok(s) = std::str::from_utf8(subj.value())
    {
        let s = s.trim().replace('\t', " ");
        let truncated = if s.len() > MAX_SUBJECT {
            &s[..MAX_SUBJECT]
        } else {
            &s
        };
        out.write_all(b" ")?;
        out.write_all(truncated.as_bytes())?;
        out.write_all(b"\n")?;
    }

    // Output folder line: "  Folder: name    size"
    let prefix = "  Folder: ";
    out.write_all(prefix.as_bytes())?;
    out.write_all(folder.as_bytes())?;

    // Calculate tab padding
    let mut col = prefix.len() + folder.len();
    col -= col % TAB_WIDTH;
    while col < LEN_OFFSET {
        out.write_all(b"\t")?;
        col += TAB_WIDTH;
    }

    // Output size
    writeln!(out, "{total_len}")?;

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
    // Handle "Name <addr>" format
    if let Some(start) = s.find('<')
        && let Some(end) = s.find('>')
    {
        return &s[start + 1..end];
    }
    // Return first word
    s.split_whitespace().next().unwrap_or("")
}

fn generate_message_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

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

    // Subject with Re: prefix
    if let Some(subj) = orig.find(b"Subject") {
        let val = subj.value();
        if let Ok(s) = std::str::from_utf8(val) {
            let s = s.trim();
            let new_subj = if s.to_ascii_lowercase().starts_with("re:") {
                s.to_string()
            } else {
                format!("Re: {}", s)
            };
            reply.push(Field::from_parts(b"Subject:", new_subj.as_bytes()));
        }
    }

    // References: combine old References + old Message-ID
    let mut refs = String::new();
    if let Some(f) = orig.find(b"References")
        && let Ok(s) = std::str::from_utf8(f.value())
    {
        refs.push_str(s.trim());
    }
    if let Some(f) = orig.find(b"Message-ID")
        && let Ok(s) = std::str::from_utf8(f.value())
    {
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
    let every = args.every || args.berkeley;
    let digest = args.digest || args.berkeley;

    // Get FILENO width from environment
    let fileno_base: i64 =
        env.get("FILENO").and_then(|s| s.parse().ok()).unwrap_or(0);
    let fileno_width = env.get("FILENO").map(|s| s.len()).unwrap_or(0);

    // Skip BABYL leader if -B
    if args.berkeley {
        skip_babyl_leader(&mut reader)?;
    }

    let mut msg_num = 0usize;
    let mut output_count = 0usize;
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
            // End of input - output last message
            if in_msg && !msg.is_empty() {
                msg_num += 1;
                if msg_num > skip && total.is_none_or(|t| output_count < t) {
                    output_message(
                        &args,
                        cmd,
                        &msg,
                        output_count,
                        fileno_base,
                        fileno_width,
                    )?;
                }
            }
            break;
        }

        // Check for BABYL separator
        if args.berkeley && line.starts_with(&[BABYL_SEP1]) {
            babyl_start = true;
            // Output previous message if any
            if in_msg && !msg.is_empty() {
                msg_num += 1;
                if msg_num > skip {
                    if let Some(t) = total
                        && output_count >= t
                    {
                        break;
                    }
                    output_message(
                        &args,
                        cmd,
                        &msg,
                        output_count,
                        fileno_base,
                        fileno_width,
                    )?;
                    output_count += 1;
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
        if args.berkeley && !babyl_start {
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
                msg_num += 1;
                if msg_num > skip {
                    if let Some(t) = total
                        && output_count >= t
                    {
                        break;
                    }
                    output_message(
                        &args,
                        cmd,
                        &msg,
                        output_count,
                        fileno_base,
                        fileno_width,
                    )?;
                    output_count += 1;
                }
            }

            // Start new message
            msg.clear();
            pending_header.clear();
            // Convert Mail-from: to From (BABYL format)
            if args.berkeley && line.len() > 10 {
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

fn output_message(
    args: &Args, cmd: &[String], msg: &[u8], num: usize, fileno_base: i64,
    fileno_width: usize,
) -> io::Result<()> {
    if cmd.is_empty() {
        // No command - output to stdout
        io::stdout().write_all(msg)?;
    } else {
        // Set FILENO and pipe to command
        let fileno = format!(
            "{:0width$}",
            fileno_base + num as i64,
            width = fileno_width.max(1)
        );
        let mut child = Command::new(&cmd[0])
            .args(&cmd[1..])
            .env("FILENO", &fileno)
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(msg)?;
        }

        if args.nowait.is_none() {
            child.wait()?;
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
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom};

    // Get the key to check - Message-ID normally, sender address with -r
    let key = if args.reply {
        find_reply_address(args, fields).unwrap_or_default()
    } else {
        fields
            .find(b"Message-ID")
            .and_then(|f| std::str::from_utf8(f.value()).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };

    // Strip leading spaces (like original)
    let key = key.trim_start();
    if key.is_empty() {
        return Ok(false);
    }

    // Open or create cache file
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache)?;

    // Read existing cache, limited to maxlen
    let mut contents = vec![0u8; maxlen];
    let bytes_read = file.read(&mut contents)?;
    contents.truncate(bytes_read);

    // Search for key and find insertion point.
    // The original algorithm: read entries until past maxlen quota.
    // If we find an empty entry (end marker), record its position.
    // If we hit maxlen without finding end marker, insert at 0.
    let mut is_dup = false;
    let mut insert_offset: Option<usize> = None;

    let mut pos = 0;
    while pos < contents.len() {
        let entry_start = pos;
        // Find end of this entry
        while pos < contents.len() && contents[pos] != 0 {
            pos += 1;
        }
        let entry = &contents[entry_start..pos];

        if entry.is_empty() {
            // Empty entry = end of buffer marker
            if insert_offset.is_none() {
                insert_offset = Some(entry_start);
            }
        } else if entry == key.as_bytes() {
            is_dup = true;
            break;
        }

        // Skip the null terminator
        if pos < contents.len() {
            pos += 1;
        }
    }

    if !is_dup {
        // Determine insertion position
        let offset = if let Some(off) = insert_offset {
            // Found end marker, insert there
            off
        } else if bytes_read >= maxlen {
            // Read full buffer but no end marker - wrap to start
            0
        } else {
            // File hasn't filled up yet, append at EOF
            bytes_read
        };

        // Check if new entry would exceed maxlen, if so wrap to 0
        let needed = key.len() + 2; // key + null + end marker
        let final_offset = if offset + needed > maxlen { 0 } else { offset };

        // Write key + null + end marker (null)
        file.seek(SeekFrom::Start(final_offset as u64))?;
        file.write_all(key.as_bytes())?;
        file.write_all(b"\0")?;
        file.write_all(b"\0")?; // End of buffer marker
    }

    Ok(is_dup)
}

fn is_header_field(line: &[u8]) -> bool {
    // Check if line looks like a header field (has colon, valid name)
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

/// Find address to reply to.
fn find_reply_address(args: &Args, fields: &FieldList) -> Option<String> {
    // With -t (trust), use header sender (Reply-To, From)
    // Without -t, use envelope sender (Return-Path, From_)
    if args.trust {
        // Header reply order
        const HEADER_FIELDS: &[&str] = &["Reply-To", "From", "Sender"];
        for name in HEADER_FIELDS {
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
        // Envelope reply order
        if let Some(f) = fields.find(b"Return-Path")
            && let Ok(s) = std::str::from_utf8(f.value())
        {
            let addr = extract_address(s.trim());
            if !addr.is_empty() && addr != "<>" {
                return Some(addr.to_string());
            }
        }
        // Check From_ line
        if let Some(f) = fields.iter().next()
            && f.is_from_line()
            && let Ok(s) = std::str::from_utf8(f.value())
        {
            let addr = s.split_whitespace().next().unwrap_or("");
            if !addr.is_empty() {
                return Some(addr.to_string());
            }
        }
        // Fall back to From:
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
