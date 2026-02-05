use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;
use std::time::UNIX_EPOCH;

use clap::Parser;
use filetime::FileTime;
use regex::Regex;
use time::format_description::OwnedFormatItem;
use time::{OffsetDateTime, UtcOffset};

use corpmail::locking::FileLock;
use corpmail::util::{EX_CANTCREAT, EX_NOINPUT, EX_OK, EX_TEMPFAIL, exit};

#[cfg(test)]
mod tests;

static MAILDIR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/new/[^/]+$").unwrap());

static DATE_FMT: LazyLock<OwnedFormatItem> = LazyLock::new(|| {
    time::format_description::parse_owned::<1>(
        "[day] [month repr:short], [hour]:[minute]",
    )
    .expect("DATE_FMT is valid")
});

#[derive(Parser)]
#[command(name = "mailstat")]
#[command(about = "Show statistics about procmail logfile")]
#[command(version)]
struct Args {
    /// Ignore errors in logfile
    #[arg(short = 'i')]
    quiet: bool,

    /// Keep logfile intact (implies -p)
    #[arg(short = 'k')]
    keep: bool,

    /// Use long display format (show averages)
    #[arg(short = 'l')]
    long: bool,

    /// Merge errors into one line
    #[arg(short = 'm')]
    merge: bool,

    /// Use old logfile (implies -k)
    #[arg(short = 'o')]
    old: bool,

    /// Preserve (append to) old logfile
    #[arg(short = 'p')]
    preserve: bool,

    /// Be silent if log is empty
    #[arg(short = 's')]
    silent: bool,

    /// Use terse display format
    #[arg(short = 't')]
    terse: bool,

    /// Ignore ignore commands in .mailstatrc
    #[arg(short = 'z')]
    raw: bool,

    /// Logfile to process
    file: PathBuf,
}

#[derive(Default)]
struct Stats {
    msgs: u64,
    bytes: u64,
}

impl Stats {
    fn add(&mut self, bytes: u64) {
        self.msgs += 1;
        self.bytes += bytes;
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    let keep = args.keep || args.old;

    let _locks = if keep {
        Vec::new()
    } else {
        match acquire_locks(&args.file) {
            Ok(l) => l,
            Err(code) => return code,
        }
    };

    exit(run(&args, keep))
}

fn run(args: &Args, keep: bool) -> u8 {
    let rc = if args.old {
        RcConfig::default()
    } else {
        load_rc()
    };
    let input = input_path(&args.file, args.old);

    let Ok(meta) = fs::metadata(&input) else {
        eprintln!("mailstat: logfile \"{}\" does not exist", input.display());
        return EX_NOINPUT;
    };

    if meta.len() == 0 {
        return print_no_mail(&meta, &rc.date_fmt, args.silent);
    }

    let mtime = FileTime::from_last_modification_time(&meta);
    let preserve = args.preserve || keep;
    let totals = match process_log(args, &input, keep, preserve) {
        Ok(t) => t,
        Err(code) => return code,
    };

    if !keep {
        truncate_and_preserve_mtime(&input, mtime);
    }

    let ignores = if args.raw {
        &HashSet::new()
    } else {
        &rc.ignores
    };
    let filtered = filter_totals(totals, ignores, args.quiet);
    let msgs: u64 = filtered.values().map(|s| s.msgs).sum();
    let bytes: u64 = filtered.values().map(|s| s.bytes).sum();

    if msgs == 0 {
        return print_no_mail(&meta, &rc.date_fmt, args.silent);
    }

    print_stats(&filtered, msgs, bytes, args.long, args.terse);
    EX_OK
}

fn input_path(base: &Path, old: bool) -> PathBuf {
    if old {
        base.with_added_extension("old")
    } else {
        base.to_path_buf()
    }
}

fn process_log(
    args: &Args, input: &Path, keep: bool, preserve: bool,
) -> Result<BTreeMap<String, Stats>, u8> {
    let reader = match File::open(input) {
        Ok(f) => BufReader::new(f),
        Err(e) => {
            eprintln!("mailstat: cannot open \"{}\": {}", input.display(), e);
            return Err(EX_NOINPUT);
        }
    };

    let mut writer = if keep {
        None
    } else {
        Some(open_old_file(&args.file, preserve)?)
    };

    let totals = parse_log(reader, writer.as_mut(), args.merge, args.old)
        .and_then(|t| {
            if let Some(ref mut w) = writer {
                w.flush()?;
            }
            Ok(t)
        });
    match totals {
        Ok(t) => Ok(t),
        Err(e) => {
            eprintln!("mailstat: write to .old file failed: {}", e);
            Err(EX_CANTCREAT)
        }
    }
}

fn open_old_file(base: &Path, preserve: bool) -> Result<BufWriter<File>, u8> {
    let path = base.with_added_extension("old");
    let file = if preserve {
        OpenOptions::new().create(true).append(true).open(&path)
    } else {
        File::create(&path)
    };
    match file {
        Ok(f) => Ok(BufWriter::new(f)),
        Err(e) => {
            eprintln!("mailstat: cannot open \"{}\": {}", path.display(), e);
            Err(EX_CANTCREAT)
        }
    }
}

fn truncate_and_preserve_mtime(path: &Path, mtime: FileTime) {
    if let Err(e) = File::create(path) {
        eprintln!("mailstat: cannot truncate \"{}\": {}", path.display(), e);
    }
    if let Err(e) = filetime::set_file_mtime(path, mtime) {
        eprintln!("mailstat: warning: cannot restore mtime: {}", e);
    }
}

fn filter_totals(
    totals: BTreeMap<String, Stats>, ignores: &HashSet<String>, quiet: bool,
) -> BTreeMap<String, Stats> {
    totals
        .into_iter()
        .filter(|(name, _)| {
            if quiet && name.starts_with("## ") {
                return false;
            }
            !ignores.contains(name)
        })
        .collect()
}

#[derive(Default)]
struct RcConfig {
    ignores: HashSet<String>,
    date_fmt: Option<OwnedFormatItem>,
}

fn load_rc() -> RcConfig {
    let Some(home) = std::env::var_os("HOME") else {
        return RcConfig::default();
    };
    let rc = Path::new(&home).join(".mailstatrc");
    let Ok(f) = File::open(rc) else {
        return RcConfig::default();
    };

    let mut ignores = HashSet::new();
    let mut date_fmt = None;
    for (lineno, line) in BufReader::new(f).lines().enumerate() {
        let Ok(line) = line else { continue };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(arg) = line.strip_prefix("ignore ") {
            ignores.insert(arg.to_string());
        } else if let Some(arg) = line.strip_prefix("date_format ") {
            match time::format_description::parse_owned::<1>(arg) {
                Ok(fmt) => date_fmt = Some(fmt),
                Err(e) => eprintln!(
                    "mailstat: bad date_format on line {} in ~/.mailstatrc: {}",
                    lineno + 1,
                    e
                ),
            }
        } else {
            eprintln!(
                "mailstat: unknown command on line {} in ~/.mailstatrc",
                lineno + 1
            );
        }
    }
    RcConfig { ignores, date_fmt }
}

fn acquire_locks(base: &Path) -> Result<Vec<FileLock>, ExitCode> {
    let mut locks = Vec::new();
    let old = base.with_added_extension("old");
    for path in [base, old.as_path()] {
        match FileLock::acquire(path) {
            Ok(l) => locks.push(l),
            Err(e) => {
                eprintln!(
                    "mailstat: cannot lock \"{}\": {}",
                    path.display(),
                    e
                );
                return Err(exit(EX_TEMPFAIL));
            }
        }
    }
    Ok(locks)
}

fn normalize_folder(name: &str) -> String {
    let name = MAILDIR_RE.replace(name, "");
    name.trim_end_matches('/').to_string()
}

fn parse_log<R, W>(
    reader: R, mut writer: Option<&mut W>, merge: bool, old: bool,
) -> io::Result<BTreeMap<String, Stats>>
where
    R: BufRead,
    W: Write,
{
    let mut totals: BTreeMap<String, Stats> = BTreeMap::new();

    for line in reader.lines() {
        let Ok(line) = line else { continue };

        if let Some(ref mut w) = writer {
            writeln!(w, "{}", line)?;
        }

        if let Some(rest) = line.strip_prefix("  Folder: ") {
            let mut parts = rest.split_whitespace();
            if let (Some(path), Some(size)) = (parts.next(), parts.next())
                && let Ok(size) = size.parse::<u64>()
            {
                let folder = normalize_folder(path);
                totals.entry(folder).or_default().add(size);
                continue;
            }
        }

        if line.starts_with("From ")
            || line
                .get(..9)
                .is_some_and(|s| s.eq_ignore_ascii_case(" subject:"))
            || line.is_empty()
            || old
        {
            continue;
        }

        let folder = if merge {
            "## diagnostic messages ##".to_string()
        } else {
            format!("## {}", line.trim())
        };
        totals.entry(folder).or_default().add(0);
    }

    Ok(totals)
}

fn print_no_mail(
    meta: &Metadata, date_fmt: &Option<OwnedFormatItem>, silent: bool,
) -> u8 {
    if silent {
        return EX_OK;
    }

    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let utc = OffsetDateTime::from_unix_timestamp(mtime as i64)
        .unwrap_or_else(|_| OffsetDateTime::now_utc());
    let local = utc
        .to_offset(UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC));
    let fmt = date_fmt.as_ref().unwrap_or(&DATE_FMT);
    let when = local.format(fmt).unwrap_or_else(|_| "unknown".to_string());

    println!("No mail arrived since {}", when);
    EX_OK
}

struct Widths {
    bytes: usize,
    avg: usize,
    msgs: usize,
    folder: usize,
}

impl Widths {
    fn compute(
        totals: &BTreeMap<String, Stats>, msgs: u64, bytes: u64,
    ) -> Self {
        let avg = totals
            .values()
            .filter(|s| s.msgs > 0)
            .map(|s| digits(s.bytes / s.msgs))
            .max()
            .unwrap_or(1);
        let folder = totals.keys().map(|s| s.len()).max().unwrap_or(0);

        Self {
            bytes: digits(bytes).max("Total".len()),
            avg: avg.max("Average".len()),
            msgs: digits(msgs).max("Number".len()),
            folder: folder.max("Folder".len()),
        }
    }
}

enum Format {
    Long,
    Short,
}

impl Format {
    fn header(&self, w: &Widths) {
        match self {
            Self::Long => println!(
                "  {:>b$}  {:>a$}  {:>m$}  Folder",
                "Total",
                "Average",
                "Number",
                b = w.bytes,
                a = w.avg,
                m = w.msgs
            ),
            Self::Short => println!(
                "  {:>b$}  {:>m$}  Folder",
                "Total",
                "Number",
                b = w.bytes,
                m = w.msgs
            ),
        }
    }

    fn sep(&self, w: &Widths) {
        match self {
            Self::Long => println!(
                "  {:->b$}  {:->a$}  {:->m$}  {:->f$}",
                "",
                "",
                "",
                "",
                b = w.bytes,
                a = w.avg,
                m = w.msgs,
                f = w.folder
            ),
            Self::Short => println!(
                "  {:->b$}  {:->m$}  {:->f$}",
                "",
                "",
                "",
                b = w.bytes,
                m = w.msgs,
                f = w.folder
            ),
        }
    }

    fn row(&self, w: &Widths, prefix: &str, s: &Stats, name: &str) {
        let avg = if s.msgs > 0 { s.bytes / s.msgs } else { 0 };
        match self {
            Self::Long => println!(
                "{}{:>b$}  {:>a$}  {:>m$}  {}",
                prefix,
                s.bytes,
                avg,
                s.msgs,
                name,
                b = w.bytes,
                a = w.avg,
                m = w.msgs
            ),
            Self::Short => println!(
                "{}{:>b$}  {:>m$}  {}",
                prefix,
                s.bytes,
                s.msgs,
                name,
                b = w.bytes,
                m = w.msgs
            ),
        }
    }

    fn footer(&self, w: &Widths, msgs: u64, bytes: u64) {
        let avg = if msgs > 0 { bytes / msgs } else { 0 };
        match self {
            Self::Long => println!(
                "  {:>b$}  {:>a$}  {:>m$}  ",
                bytes,
                avg,
                msgs,
                b = w.bytes,
                a = w.avg,
                m = w.msgs
            ),
            Self::Short => println!(
                "  {:>b$}  {:>m$}  ",
                bytes,
                msgs,
                b = w.bytes,
                m = w.msgs
            ),
        }
    }
}

fn print_stats(
    totals: &BTreeMap<String, Stats>, msgs: u64, bytes: u64, long: bool,
    terse: bool,
) {
    let w = Widths::compute(totals, msgs, bytes);
    let fmt = if long { Format::Long } else { Format::Short };

    if !terse {
        println!();
        fmt.header(&w);
        fmt.sep(&w);
    }

    let prefix = if terse { "" } else { "  " };
    for (name, s) in totals {
        fmt.row(&w, prefix, s, name);
    }

    if !terse {
        fmt.sep(&w);
        fmt.footer(&w, msgs, bytes);
    }
}

fn digits(n: u64) -> usize {
    if n == 0 {
        1
    } else {
        (n as f64).log10().floor() as usize + 1
    }
}
