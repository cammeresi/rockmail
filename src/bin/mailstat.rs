use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use clap::Parser;
use corpmail::locking::{create_lock, remove_lock};
use corpmail::util::{EX_CANTCREAT, EX_NOINPUT, EX_OK, exit};
use regex::Regex;

static MAILDIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/new/[-0-9A-Za-z_][-0-9A-Za-z_.,+:%@]*$").unwrap()
});

#[derive(Parser)]
#[command(name = "mailstat")]
#[command(about = "Show statistics about procmail logfile")]
#[command(version)]
struct Args {
    /// Ignore errors in logfile
    #[arg(short = 'i')]
    ignore_errors: bool,

    /// Keep logfile intact (implies -p)
    #[arg(short = 'k')]
    keep: bool,

    /// Use long display format (show averages)
    #[arg(short = 'l')]
    long: bool,

    /// Merge errors into one line
    #[arg(short = 'm')]
    merge_errors: bool,

    /// Use old logfile (implies -k)
    #[arg(short = 'o')]
    use_old: bool,

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
    no_ignore: bool,

    /// Logfile to process
    file: PathBuf,
}

struct Stats {
    msgs: u64,
    bytes: u64,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let keep = args.keep || args.use_old;
    let preserve = args.preserve || keep;

    let ignores = if !args.use_old && !args.no_ignore {
        load_ignores()
    } else {
        Vec::new()
    };

    let input = input_path(&args.file, args.use_old);

    let Ok(meta) = fs::metadata(&input) else {
        eprintln!("mailstat: logfile `{}' does not exist", input.display());
        return exit(EX_NOINPUT);
    };
    let mtime = filetime::FileTime::from_last_modification_time(&meta);

    if meta.len() == 0 {
        if !args.silent {
            print_no_mail(&args.file);
        }
        return exit(EX_OK);
    }

    let locks = if keep {
        Vec::new()
    } else {
        acquire_locks(&args.file)
    };

    let result = process_log(&args, &input, keep, preserve, &locks);
    let totals = match result {
        Ok(t) => t,
        Err(code) => return code,
    };

    if !keep {
        truncate_and_preserve_mtime(&input, mtime);
    }
    release_locks(&locks);

    let filtered = filter_totals(totals, &ignores, args.ignore_errors);
    let total_msgs: u64 = filtered.values().map(|s| s.msgs).sum();
    let total_bytes: u64 = filtered.values().map(|s| s.bytes).sum();

    if total_msgs == 0 {
        if !args.silent {
            print_no_mail(&args.file);
        }
        return exit(EX_OK);
    }

    print_stats(&filtered, total_msgs, total_bytes, args.long, args.terse);
    exit(EX_OK)
}

fn input_path(base: &Path, use_old: bool) -> PathBuf {
    if use_old {
        with_suffix(base, ".old")
    } else {
        base.to_path_buf()
    }
}

fn process_log(
    args: &Args, input: &Path, keep: bool, preserve: bool, locks: &[PathBuf],
) -> Result<BTreeMap<String, Stats>, ExitCode> {
    let reader = match File::open(input) {
        Ok(f) => BufReader::new(f),
        Err(e) => {
            eprintln!("mailstat: cannot open `{}': {}", input.display(), e);
            release_locks(locks);
            return Err(exit(EX_NOINPUT));
        }
    };

    let mut writer = if keep {
        None
    } else {
        Some(open_old_file(&args.file, preserve, locks)?)
    };

    let totals =
        parse_log(reader, writer.as_mut(), args.merge_errors, args.use_old);

    if let Some(ref mut w) = writer {
        let _ = w.flush();
    }
    Ok(totals)
}

fn open_old_file(
    base: &Path, preserve: bool, locks: &[PathBuf],
) -> Result<BufWriter<File>, ExitCode> {
    let path = with_suffix(base, ".old");
    let file = if preserve {
        OpenOptions::new().create(true).append(true).open(&path)
    } else {
        File::create(&path)
    };
    match file {
        Ok(f) => Ok(BufWriter::new(f)),
        Err(e) => {
            eprintln!("mailstat: cannot open `{}': {}", path.display(), e);
            release_locks(locks);
            Err(exit(EX_CANTCREAT))
        }
    }
}

fn truncate_and_preserve_mtime(path: &Path, mtime: filetime::FileTime) {
    if let Err(e) = File::create(path) {
        eprintln!("mailstat: cannot truncate `{}': {}", path.display(), e);
    }
    let _ = filetime::set_file_mtime(path, mtime);
}

fn filter_totals(
    totals: BTreeMap<String, Stats>, ignores: &[String], ignore_errors: bool,
) -> BTreeMap<String, Stats> {
    totals
        .into_iter()
        .filter(|(name, _)| {
            if ignore_errors && name.starts_with("## ") {
                return false;
            }
            !ignores.iter().any(|i| i == name)
        })
        .collect()
}

fn load_ignores() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let rc = Path::new(&home).join(".mailstatrc");
    let Ok(f) = File::open(rc) else {
        return Vec::new();
    };

    let mut ignores = Vec::new();
    for (lineno, line) in BufReader::new(f).lines().enumerate() {
        let Ok(line) = line else { continue };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(arg) = line.strip_prefix("ignore ") {
            ignores.push(arg.to_string());
        } else {
            eprintln!(
                "mailstat: unknown command on line {} in ~/.mailstatrc",
                lineno + 1
            );
        }
    }
    ignores
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

fn acquire_locks(base: &Path) -> Vec<PathBuf> {
    let mut locks = Vec::new();
    for suffix in [".lock", ".old.lock"] {
        let path = with_suffix(base, suffix);
        if create_lock(&path).is_ok() {
            locks.push(path);
        }
    }
    locks
}

fn release_locks(locks: &[PathBuf]) {
    for lock in locks {
        let _ = remove_lock(lock);
    }
}

fn normalize_folder(name: &str) -> String {
    // Strip Maildir delivery suffix: /new/unique-filename
    let name = MAILDIR_RE.replace(name, "");
    // Strip trailing slash (mbox delivered as maildir hack)
    name.trim_end_matches('/').to_string()
}

fn parse_log<R, W>(
    reader: R, mut writer: Option<&mut W>, merge: bool, use_old: bool,
) -> BTreeMap<String, Stats>
where
    R: BufRead,
    W: Write,
{
    let mut totals: BTreeMap<String, Stats> = BTreeMap::new();

    for line in reader.lines() {
        let Ok(line) = line else { continue };

        if let Some(ref mut w) = writer {
            let _ = writeln!(w, "{}", line);
        }

        // Look for "  Folder: <path> <size>"
        if let Some(rest) = line.strip_prefix("  Folder: ") {
            let parts: Vec<_> = rest.split_whitespace().collect();
            if parts.len() >= 2
                && let Ok(size) = parts[1].parse::<u64>()
            {
                let folder = normalize_folder(parts[0]);
                let entry =
                    totals.entry(folder).or_insert(Stats { msgs: 0, bytes: 0 });
                entry.msgs += 1;
                entry.bytes += size;
                continue;
            }
        }

        // Skip known informational lines
        if line.starts_with("From ")
            || line.to_ascii_lowercase().starts_with(" subject:")
            || line.is_empty()
        {
            continue;
        }

        // When reading old logfile, skip unrecognized lines silently
        if use_old {
            continue;
        }

        // Error/diagnostic line
        let folder = if merge {
            "## diagnostic messages ##".to_string()
        } else {
            format!("## {}", line.trim())
        };
        let entry = totals.entry(folder).or_insert(Stats { msgs: 0, bytes: 0 });
        entry.msgs += 1;
    }

    totals
}

fn print_no_mail(path: &Path) {
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let tm = time::OffsetDateTime::from_unix_timestamp(mtime as i64)
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let fmt = time::format_description::parse(
        "[day] [month repr:short], [hour].[minute]",
    )
    .unwrap();
    let when = tm.format(&fmt).unwrap_or_else(|_| "unknown".to_string());

    println!("No mail arrived since {}", when);
}

struct Widths {
    bytes: usize,
    avg: usize,
    msgs: usize,
    folder: usize,
}

impl Widths {
    fn compute(
        totals: &BTreeMap<String, Stats>, total_msgs: u64, total_bytes: u64,
    ) -> Self {
        let max_avg = totals
            .values()
            .filter(|s| s.msgs > 0)
            .map(|s| digits(s.bytes / s.msgs))
            .max()
            .unwrap_or(1);
        let max_folder = totals.keys().map(|s| s.len()).max().unwrap_or(0);

        Self {
            bytes: digits(total_bytes).max("Total".len()),
            avg: max_avg.max("Average".len()),
            msgs: digits(total_msgs).max("Number".len()),
            folder: max_folder.max("Folder".len()),
        }
    }

    fn print_sep(&self, long: bool) {
        if long {
            println!(
                "  {:->b$}  {:->a$}  {:->m$}  {:->f$}",
                "",
                "",
                "",
                "",
                b = self.bytes,
                a = self.avg,
                m = self.msgs,
                f = self.folder
            );
        } else {
            println!(
                "  {:->b$}  {:->m$}  {:->f$}",
                "",
                "",
                "",
                b = self.bytes,
                m = self.msgs,
                f = self.folder
            );
        }
    }
}

fn print_stats(
    totals: &BTreeMap<String, Stats>, total_msgs: u64, total_bytes: u64,
    long: bool, terse: bool,
) {
    let w = Widths::compute(totals, total_msgs, total_bytes);

    if !terse {
        println!();
        if long {
            println!(
                "  {:>b$}  {:>a$}  {:>m$}  Folder",
                "Total",
                "Average",
                "Number",
                b = w.bytes,
                a = w.avg,
                m = w.msgs
            );
        } else {
            println!(
                "  {:>b$}  {:>m$}  Folder",
                "Total",
                "Number",
                b = w.bytes,
                m = w.msgs
            );
        }
        w.print_sep(long);
    }

    let prefix = if terse { "" } else { "  " };
    for (name, s) in totals {
        let avg = if s.msgs > 0 { s.bytes / s.msgs } else { 0 };
        if long {
            println!(
                "{}{:>b$}  {:>a$}  {:>m$}  {}",
                prefix,
                s.bytes,
                avg,
                s.msgs,
                name,
                b = w.bytes,
                a = w.avg,
                m = w.msgs
            );
        } else {
            println!(
                "{}{:>b$}  {:>m$}  {}",
                prefix,
                s.bytes,
                s.msgs,
                name,
                b = w.bytes,
                m = w.msgs
            );
        }
    }

    if !terse {
        w.print_sep(long);
        let avg = if total_msgs > 0 {
            total_bytes / total_msgs
        } else {
            0
        };
        if long {
            println!(
                "  {:>b$}  {:>a$}  {:>m$}  ",
                total_bytes,
                avg,
                total_msgs,
                b = w.bytes,
                a = w.avg,
                m = w.msgs
            );
        } else {
            println!(
                "  {:>b$}  {:>m$}  ",
                total_bytes,
                total_msgs,
                b = w.bytes,
                m = w.msgs
            );
        }
    }
}

fn digits(n: u64) -> usize {
    if n == 0 {
        1
    } else {
        (n as f64).log10().floor() as usize + 1
    }
}
