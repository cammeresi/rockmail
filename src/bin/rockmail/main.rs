//! Corpmail binary - autonomous mail processor (procmail replacement).

use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, ErrorKind, IsTerminal, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use miette::MietteHandlerOpts;
use nix::unistd::{ROOT, Uid, User};
use rockmail::config::{self, is_var_name};
use rockmail::delivery::{DeliveryOpts, FolderType};
use rockmail::engine::{Engine, Outcome};
use rockmail::mail::Message;
use rockmail::util::{
    EX_CANTCREAT, EX_TEMPFAIL, EX_USAGE, init_umask, signals,
};
use rockmail::variables::*;

#[cfg(test)]
mod tests;

const MAIL_SPOOL: &str = "/var/mail";
const MAX_MAIL_SIZE: u64 = 1024 * 1024 * 1024; // 1 GB
const MAX_RCFILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

/// A validated rcfile with an open file handle.
#[derive(Debug)]
struct ValidatedRcfile {
    path: PathBuf,
    file: File,
}

#[derive(Parser, Debug)]
#[command(
    name = "rockmail",
    about = "Autonomous mail processor",
    version = rockmail::VERSION,
    disable_help_flag = true,
    disable_version_flag = true,
)]
struct Args {
    /// Display version and exit
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Display help and exit
    #[arg(short = 'h', short_alias = '?')]
    help: bool,

    /// Preserve environment
    #[arg(short = 'p')]
    preserve: bool,

    /// Return EX_TEMPFAIL on error
    #[arg(short = 't')]
    tempfail: bool,

    /// Set From_ line sender
    #[arg(short = 'f', short_alias = 'r', value_name = "SENDER")]
    from: Option<String>,

    /// Dry-run mode (no folder delivery or locking)
    #[arg(short = 'n')]
    dryrun: bool,

    /// Override From_ fakes
    #[arg(short = 'o')]
    override_from: bool,

    /// Positional argument ($1, $2, ...)
    #[arg(short = 'a', action = clap::ArgAction::Append, value_name = "ARG")]
    args: Vec<String>,

    /// Variable assignments and rcfiles
    #[arg(trailing_var_arg = true)]
    rest: Vec<String>,
}

fn print_version() {
    println!(
        "rockmail {} (a Rust translation of procmail)",
        rockmail::VERSION
    );
}

fn print_help() {
    println!(
        "Usage: rockmail [-pto] [-f sender] [parameter=value | rcfile] ...
       rockmail [-to] [-f sender] [-a arg] ... -d recipient ...
       rockmail [-pt] -m [parameter=value] ... rcfile [arg] ...
       rockmail -v

Options:
  -v          Display version and exit
  -h, -?      Display this help
  -p          Preserve old environment
  -t          Return EX_TEMPFAIL on error
  -n          Dry-run: show what would be delivered without writing
  -f sender   Regenerate From_ line with sender
  -o          Override fake From_ lines
  -a arg      Set $1, $2, ... (can be repeated)

Recipe flags: HBDaAeEfhbcwWir"
    );
}

fn read_mail() -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(64 * 1024);
    io::stdin().take(MAX_MAIL_SIZE + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_MAIL_SIZE {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("mail exceeds {} byte limit", MAX_MAIL_SIZE),
        ));
    }
    Ok(buf)
}

fn read_file(mut file: File) -> io::Result<String> {
    let meta = file.metadata()?;
    if meta.len() > MAX_RCFILE_SIZE {
        return Err(io::Error::new(ErrorKind::InvalidData, "rcfile too large"));
    }
    let mut content = String::with_capacity(meta.len() as usize);
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn parse_rest(rest: &[String]) -> (Vec<(String, String)>, Vec<String>) {
    let mut assigns = Vec::new();
    let mut files = Vec::new();

    for arg in rest {
        if let Some(eq) = arg.find('=') {
            let name = &arg[..eq];
            let value = &arg[eq + 1..];
            if is_var_name(name) {
                assigns.push((name.to_string(), value.to_string()));
                continue;
            }
        }
        files.push(arg.clone());
    }

    (assigns, files)
}

fn resolve_rcpath(path: &str, home: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || path.starts_with("./") {
        p.to_path_buf()
    } else {
        PathBuf::from(home).join(path)
    }
}

/// Check that a directory is not world writable (unless sticky).
fn check_dir_security(path: &Path) -> Result<(), Box<dyn Error>> {
    let meta = fs::metadata(path)?;
    let mode = meta.mode();

    // World writable without sticky bit is unsafe
    if mode & 0o002 != 0 && mode & 0o1000 == 0 {
        return Err(
            format!("directory is world writable: {}", path.display()).into()
        );
    }

    Ok(())
}

/// Check that an rcfile has safe permissions (using fstat on open handle).
fn check_rcfile_security(
    file: &File, path: &Path, is_default: bool,
) -> Result<(), Box<dyn Error>> {
    let meta = file.metadata()?;
    let mode = meta.mode();
    let owner = meta.uid();

    // Must be owned by user or root
    if owner != Uid::current().as_raw() && owner != ROOT.as_raw() {
        return Err(format!(
            "rcfile not owned by user or root: {}",
            path.display()
        )
        .into());
    }

    // Must not be world writable
    if mode & 0o002 != 0 {
        return Err(
            format!("rcfile is world writable: {}", path.display()).into()
        );
    }

    // Default rcfile must not be group writable
    if is_default && mode & 0o020 != 0 {
        return Err(
            format!("rcfile is group writable: {}", path.display()).into()
        );
    }

    // Check all ancestor directories (for absolute paths)
    if path.is_absolute() {
        for ancestor in path.ancestors().skip(1) {
            if ancestor.as_os_str().is_empty() {
                break;
            }
            check_dir_security(ancestor)?;
        }
    }

    Ok(())
}

/// Open file and check security on the open handle.
fn open_and_check(
    path: &Path, is_default: bool,
) -> Result<Option<ValidatedRcfile>, Box<dyn Error>> {
    // Check for symlinks before opening
    let link_meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if is_default && e.kind() == ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(e) => return Err(format!("{}: {}", path.display(), e).into()),
    };
    if link_meta.file_type().is_symlink() {
        return Err(format!("rcfile is a symlink: {}", path.display()).into());
    }

    let file =
        File::open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    check_rcfile_security(&file, path, is_default)?;

    Ok(Some(ValidatedRcfile {
        path: path.to_path_buf(),
        file,
    }))
}

fn find_rcfile(
    files: &[String], engine: &Engine,
) -> Result<Option<ValidatedRcfile>, Box<dyn Error>> {
    if files.len() > 1 {
        return Err("too many rc files on command line".into());
    }

    let home = engine.get_var(VAR_HOME).unwrap_or("");

    if let Some(f) = files.first() {
        let path = resolve_rcpath(f, home);
        open_and_check(&path, false)
    } else {
        let path = PathBuf::from(home).join(".procmailrc");
        open_and_check(&path, true)
    }
}

fn deliver_default(
    engine: &mut Engine, msg: &Message,
) -> Result<String, Box<dyn Error>> {
    let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");

    for name in [VAR_DEFAULT, VAR_ORGMAIL] {
        let path = engine.get_var(name).unwrap_or("").to_owned();
        if !path.is_empty() {
            let (ft, stripped) = FolderType::parse(&path);
            let r = ft.deliver(
                Path::new(stripped),
                msg,
                sender,
                DeliveryOpts::default(),
                engine.namer(),
            )?;
            return Ok(r.path);
        }
    }

    Err("No delivery destination".into())
}

/// Check for EXITCODE variable override.
fn exit_code(engine: &Engine) -> Option<u8> {
    if !engine.exit_was_set() {
        return None;
    }
    let v = engine.get_var(VAR_EXITCODE)?;
    v.parse::<u8>().ok()
}

/// Build default environment.
///
/// SAFETY: Must be called before any threads start.
unsafe fn init_env(
    args: &Args, assignments: &[(String, String)],
) -> Environment {
    if !args.preserve {
        unsafe {
            env::vars().for_each(|(k, _)| {
                if k != VAR_TZ {
                    env::remove_var(k)
                }
            });
        }
    }

    let mut env = Environment::from_process();

    let uid = nix::unistd::getuid();
    let (logname, home, shell) = if let Ok(Some(u)) = User::from_uid(uid) {
        (
            u.name,
            u.dir.to_string_lossy().into_owned(),
            u.shell.to_string_lossy().into_owned(),
        )
    } else {
        (format!("#{}", uid), "/".into(), SHELL.def.unwrap().into())
    };

    let maildir = format!("{home}/{}", MAILDIR.def.unwrap());
    let orgmail = format!("{MAIL_SPOOL}/{logname}");
    let host = nix::unistd::gethostname()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    env.set(VAR_HOME, &home);
    env.set(VAR_LOGNAME, &logname);
    env.set(VAR_SHELL, &shell);
    env.set(VAR_MAILDIR, &maildir);
    env.set(VAR_ORGMAIL, &orgmail);
    env.set(VAR_DEFAULT, &orgmail);
    env.set(VAR_HOST, &host);
    env.set(VAR_USER_SHELL, &shell);
    env.set(VAR_PROCMAIL_VERSION, rockmail::VERSION);

    env.set_all_defaults();

    for (name, value) in assignments {
        env.set(name, value);
    }

    env
}

fn run(
    env: Environment, args: Args, rcfiles: &[String],
) -> Result<Option<u8>, Box<dyn Error>> {
    let mail = read_mail()?;
    let mut msg = Message::parse(&mail);
    let mut delivered = false;

    if let Some(ref sender) = args.from
        && (args.override_from || msg.from_line().is_none())
    {
        if sender == "-" {
            let invoker = env.get(VAR_LOGNAME).unwrap_or_default().to_owned();
            msg.refresh_envelope_sender(&invoker);
        } else {
            msg.set_envelope_sender(sender);
        }
    }

    let ctx = SubstCtx::new(args.args.clone());
    let mut engine = Engine::new(env, ctx);

    if engine.get_var(VAR_VERBOSE).is_some_and(value_is_true) {
        engine.set_verbose(true);
    }
    if args.dryrun {
        engine.set_dryrun(true);
    }

    let rcfile = find_rcfile(rcfiles, &engine)?;

    if let Some(rc) = rcfile {
        let content = read_file(rc.file)?;
        let name = rc.path.display().to_string();
        let items = config::parse(&content, &name)?;
        engine.set_var("_", &name);
        engine.set_rcfile(&name);

        match engine.process(&items, &mut msg)? {
            Outcome::Delivered(_) => delivered = true,
            Outcome::Default | Outcome::Continue => {}
        }
    }

    if !delivered && engine.get_var(VAR_DELIVERED).is_some_and(value_is_true) {
        delivered = true;
    }

    if !delivered && engine.dryrun() {
        for name in [VAR_DEFAULT, VAR_ORGMAIL] {
            let path = engine.get_var(name).unwrap_or("").to_owned();
            if !path.is_empty() {
                eprintln!("deliver to default {}", path);
                break;
            }
        }
    }

    if engine.dryrun() {
        eprintln!("-----");
        io::stdout().write_all(msg.as_bytes())?;
    } else if !delivered {
        let folder = deliver_default(&mut engine, &msg)?;
        engine.log_abstract(&folder, &msg);
    } else {
        let folder = engine.get_var(VAR_LASTFOLDER).unwrap_or("").to_owned();
        engine.log_abstract(&folder, &msg);
    }

    engine.run_trap(&msg);

    Ok(exit_code(&engine))
}

fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EX_USAGE);
        }
    };

    if args.version {
        print_version();
        return ExitCode::SUCCESS;
    } else if args.help {
        print_help();
        return ExitCode::SUCCESS;
    }

    // Snapshot terminal capabilities before clearing the environment.
    let term = io::stderr().is_terminal();
    let opts = MietteHandlerOpts::new()
        .color(term)
        .unicode(term)
        .terminal_links(false);
    miette::set_hook(Box::new(move |_| Box::new(opts.clone().build()))).ok();

    signals::setup();
    init_umask();

    let fail_code = if args.tempfail {
        EX_TEMPFAIL
    } else {
        EX_CANTCREAT
    };

    let (assignments, rcfiles) = parse_rest(&args.rest);

    // SAFETY:
    // If some day there should be thread spawning, threads must not be
    // spawned prior to environment setup.
    let env = unsafe { init_env(&args, &assignments) };

    match run(env, args, &rcfiles) {
        Ok(code) => code.map_or(ExitCode::SUCCESS, ExitCode::from),
        Err(e) => {
            eprintln!("rockmail: {e}");
            ExitCode::from(fail_code)
        }
    }
}
