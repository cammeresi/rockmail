//! Corpmail binary - autonomous mail processor (procmail replacement).

use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use corpmail::config::{self, is_var_name};
use corpmail::delivery::{DeliveryOpts, FolderType};
use corpmail::mail::Message;
use corpmail::recipe::{Engine, Outcome};
use corpmail::util::{EX_CANTCREAT, EX_TEMPFAIL, EX_USAGE, signals};
use corpmail::variables::*;
use nix::unistd::{ROOT, Uid, User};

#[cfg(test)]
mod tests;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ETC_PROCMAILRC: &str = "/etc/procmailrc";
const MAIL_SPOOL: &str = "/var/mail";
const ROOT_UID: u32 = 0;

/// A validated rcfile with an open file handle.
#[derive(Debug)]
struct ValidatedRcfile {
    path: PathBuf,
    file: File,
}

#[derive(Parser, Debug)]
#[command(
    name = "corpmail",
    about = "Autonomous mail processor",
    version = VERSION,
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

    signals::setup();

    let fail_code = if args.tempfail {
        EX_TEMPFAIL
    } else {
        EX_CANTCREAT
    };

    let penv = init_env(&args);

    // SAFETY:
    // If some day there should be thread spawning, threads must not be
    // spawned prior to the preceding environment setup.

    match run(args, penv) {
        Ok(code) => code.map_or(ExitCode::SUCCESS, ExitCode::from),
        Err(e) => {
            eprintln!("corpmail: {e}");
            ExitCode::from(fail_code)
        }
    }
}

fn run(args: Args, penv: ProcEnv) -> Result<Option<u8>, Box<dyn Error>> {
    let mail = read_mail()?;
    let mut msg = Message::parse(&mail);
    let mut delivered = false;

    if let Some(ref sender) = args.from
        && (args.override_from || msg.from_line().is_none())
    {
        msg.set_envelope_sender(sender);
    }

    let ctx = SubstCtx::new(args.args);
    let mut engine = Engine::new(RealEnv, ctx);

    // FIXME assignments in rc file
    if env::var(VAR_VERBOSE).is_ok_and(value_is_true) {
        engine.set_verbose(true);
    }

    let (_, rcfiles) = parse_rest(&args.rest);

    // Process /etc/procmailrc if not in preserve mode (-p), no rcfile on
    // command line.
    let has_cmdline_rcfile = !rcfiles.is_empty();
    if !args.preserve
        && !has_cmdline_rcfile
        && let Some(Outcome::Delivered(_)) =
            process_etcrc(&mut engine, &mut msg)?
    {
        return Ok(exit_code(&engine));
    }

    // Find user rcfile to use
    let rcfile = find_rcfile(&rcfiles, &penv)?;

    if let Some(rc) = rcfile {
        let content = read_file(rc.file)?;
        let items = config::parse(&content)?;
        engine.set_var("_", &rc.path.display().to_string());

        match engine.process(&items, &mut msg)? {
            Outcome::Delivered(_) => delivered = true,
            Outcome::Default | Outcome::Continue => {}
        }
    }

    // DELIVERED variable: pretend message was delivered
    if !delivered
        && engine
            .get_var(VAR_DELIVERED)
            .is_some_and(|v| v == "yes" || v == "1")
    {
        delivered = true;
    }

    if !delivered {
        deliver_default(&mut engine, &penv, &msg)?;
    }

    Ok(exit_code(&engine))
}

/// Check for EXITCODE variable override.
fn exit_code(engine: &Engine<RealEnv>) -> Option<u8> {
    let v = engine.get_var(VAR_EXITCODE)?;
    v.parse::<u8>().ok()
}

/// Process /etc/procmailrc if it exists.
fn process_etcrc(
    engine: &mut Engine<RealEnv>, msg: &mut Message,
) -> Result<Option<Outcome>, Box<dyn Error>> {
    let path = Path::new(ETC_PROCMAILRC);

    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    check_etcrc_security(&file, path)?;

    let content = read_file(file)?;
    let items = config::parse(&content)?;
    engine.set_var("_", ETC_PROCMAILRC);

    let outcome = engine.process(&items, msg)?;
    Ok(Some(outcome))
}

/// Check that /etc/procmailrc has safe permissions.
fn check_etcrc_security(
    file: &File, path: &Path,
) -> Result<(), Box<dyn Error>> {
    let meta = file.metadata()?;
    let mode = meta.mode();
    let owner = meta.uid();

    // Must be owned by root
    if owner != ROOT_UID {
        return Err(format!(
            "system rcfile not owned by root: {}",
            path.display()
        )
        .into());
    }

    // Must not be world writable
    if mode & 0o002 != 0 {
        return Err(format!(
            "system rcfile is world writable: {}",
            path.display()
        )
        .into());
    }

    Ok(())
}

const MAX_MAIL_SIZE: u64 = 1024 * 1024 * 1024; // 1 GB
const MAX_RCFILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

fn read_file(mut file: File) -> io::Result<String> {
    let meta = file.metadata()?;
    if meta.len() > MAX_RCFILE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "rcfile too large",
        ));
    }
    let mut content = String::with_capacity(meta.len() as usize);
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn read_mail() -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(64 * 1024);
    io::stdin().take(MAX_MAIL_SIZE + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_MAIL_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mail exceeds {} byte limit", MAX_MAIL_SIZE),
        ));
    }
    Ok(buf)
}

/// Initialize process environment.  Must be called before any threads spawn.
fn init_env(args: &Args) -> ProcEnv {
    let penv = build_env(args);

    // Set user variable assignments into environment
    let (assignments, _) = parse_rest(&args.rest);
    for (name, value) in assignments {
        // SAFETY: called before any threads spawn
        unsafe {
            env::set_var(&name, &value);
        }
    }

    penv
}

fn build_env(args: &Args) -> ProcEnv {
    let mut e = ProcEnv::default();

    let uid = nix::unistd::getuid();
    if let Some(user) = get_user_by_uid(uid.as_raw()) {
        e.logname = user.name;
        e.home = user.home;
        e.shell = user.shell;
    } else {
        e.logname = format!("#{}", uid);
        e.home = "/".into();
        e.shell = "/bin/sh".into();
    }

    e.maildir = format!("{}/Mail", e.home);
    e.orgmail = format!("{}/{}", MAIL_SPOOL, e.logname);

    if let Ok(name) = nix::unistd::gethostname() {
        e.host = name.to_string_lossy().into_owned();
    }

    // SAFETY: called before any threads spawn
    unsafe {
        if !args.preserve {
            env::vars().for_each(|(k, _)| {
                if k != VAR_TZ {
                    env::remove_var(k);
                }
            });
        }

        env::set_var(VAR_HOME, &e.home);
        env::set_var(VAR_LOGNAME, &e.logname);
        env::set_var(VAR_SHELL, &e.shell);
        env::set_var(VAR_PATH, DEF_PATH);
        env::set_var(VAR_MAILDIR, &e.maildir);
        env::set_var(VAR_ORGMAIL, &e.orgmail);
        env::set_var(VAR_DEFAULT, &e.orgmail);
        env::set_var(VAR_HOST, &e.host);
        env::set_var(VAR_SENDMAILFLAGS, DEF_SENDMAILFLAGS);
        env::set_var(VAR_PROCMAIL_VERSION, VERSION);
        env::set_var(VAR_USER_SHELL, &e.shell);
        env::set_var(VAR_NORESRETRY, DEF_NORESRETRY.to_string());
        env::set_var(VAR_SUSPEND, DEF_SUSPEND.to_string());
        env::set_var(VAR_LOGABSTRACT, DEF_LOGABSTRACT.to_string());
    }

    e
}

/// Process environment for mail delivery.
#[derive(Default)]
struct ProcEnv {
    /// User's login name.
    logname: String,
    /// User's home directory.
    home: String,
    /// User's shell.
    shell: String,
    /// User's mail directory (~/Mail).
    maildir: String,
    /// System mailbox (/var/mail/$LOGNAME).
    orgmail: String,
    /// System hostname.
    host: String,
}

/// User information from passwd database.
struct UserInfo {
    name: String,
    home: String,
    shell: String,
}

fn get_user_by_uid(uid: u32) -> Option<UserInfo> {
    let user = User::from_uid(Uid::from_raw(uid)).ok()??;
    Some(UserInfo {
        name: user.name,
        home: user.dir.to_string_lossy().into_owned(),
        shell: user.shell.to_string_lossy().into_owned(),
    })
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

fn find_rcfile(
    files: &[String], env: &ProcEnv,
) -> Result<Option<ValidatedRcfile>, Box<dyn Error>> {
    // Command line rcfiles
    for f in files {
        let path = resolve_rcpath(f, env);
        if let Some(rc) = open_and_check(&path, false)? {
            return Ok(Some(rc));
        }
    }

    // Default: ~/.procmailrc
    if files.is_empty() {
        let default = PathBuf::from(&env.home).join(".procmailrc");
        if let Some(rc) = open_and_check(&default, true)? {
            return Ok(Some(rc));
        }
    }

    Ok(None)
}

/// Open file and check security on the open handle.
fn open_and_check(
    path: &Path, is_default: bool,
) -> Result<Option<ValidatedRcfile>, Box<dyn Error>> {
    // Check for symlinks before opening
    let link_meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if link_meta.file_type().is_symlink() {
        return Err(format!("rcfile is a symlink: {}", path.display()).into());
    }

    let file = File::open(path)?;
    check_rcfile_security(&file, path, is_default)?;
    Ok(Some(ValidatedRcfile {
        path: path.to_path_buf(),
        file,
    }))
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

fn resolve_rcpath(path: &str, env: &ProcEnv) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || path.starts_with("./") {
        p.to_path_buf()
    } else {
        PathBuf::from(&env.home).join(path)
    }
}

fn deliver_default<E>(
    engine: &mut Engine<E>, penv: &ProcEnv, msg: &Message,
) -> Result<(), Box<dyn Error>>
where
    E: Env,
{
    let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");

    for name in [VAR_DEFAULT, VAR_ORGMAIL] {
        let path = engine
            .get_var(name)
            .map(|v| v.into_owned())
            .unwrap_or_else(|| penv.orgmail.clone());
        if !path.is_empty() {
            let (ft, stripped) = FolderType::parse(&path);
            ft.deliver(
                Path::new(stripped),
                msg,
                sender,
                DeliveryOpts::default(),
                engine.namer(),
            )?;
            return Ok(());
        }
    }

    Err("No delivery destination".into())
}

fn print_version() {
    println!("corpmail v{VERSION} (a Rust translation of procmail)");
}

fn print_help() {
    println!(
        "Usage: corpmail [-pto] [-f sender] [parameter=value | rcfile] ...
       corpmail [-to] [-f sender] [-a arg] ... -d recipient ...
       corpmail [-pt] -m [parameter=value] ... rcfile [arg] ...
       corpmail -v

Options:
  -v          Display version and exit
  -h, -?      Display this help
  -p          Preserve old environment
  -t          Return EX_TEMPFAIL on error
  -f sender   Regenerate From_ line with sender
  -o          Override fake From_ lines
  -a arg      Set $1, $2, ... (can be repeated)

Recipe flags: HBDaAeEfhbcwWir"
    );
}
