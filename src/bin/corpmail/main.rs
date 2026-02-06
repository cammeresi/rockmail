//! Corpmail binary - autonomous mail processor (procmail replacement).

use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use corpmail::config::{self, is_var_name};
use corpmail::mail::Message;
use corpmail::recipe::{Engine, Outcome};
use corpmail::util::{EX_CANTCREAT, EX_TEMPFAIL, EX_USAGE, signals};
use corpmail::variables::*;
use nix::unistd::{Uid, User};

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

    /// Delivery mode (deliver to named recipient)
    #[arg(short = 'd', value_name = "RECIPIENT")]
    deliver: Option<String>,

    /// Mail filter mode
    #[arg(short = 'm')]
    mailfilter: bool,

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
    }

    if args.help {
        print_help();
        return ExitCode::SUCCESS;
    }

    if args.deliver.is_some() && (args.preserve || args.mailfilter) {
        eprintln!("corpmail: Conflicting options");
        return ExitCode::from(EX_USAGE);
    }

    if args.mailfilter && !args.args.is_empty() && !args.rest.is_empty() {
        eprintln!("corpmail: -m with -a: use trailing args instead");
        return ExitCode::from(EX_USAGE);
    }

    signals::setup();

    let fail_code = if args.tempfail {
        EX_TEMPFAIL
    } else {
        EX_CANTCREAT
    };

    // Set up environment before spawning any threads
    let penv = match init_env(&args) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("corpmail: {e}");
            return ExitCode::from(fail_code);
        }
    };

    match run(args, penv) {
        Ok(code) => code.map_or(ExitCode::SUCCESS, ExitCode::from),
        Err(e) => {
            eprintln!("corpmail: {e}");
            ExitCode::from(fail_code)
        }
    }
}

fn run(
    args: Args, penv: ProcEnv,
) -> Result<Option<u8>, Box<dyn std::error::Error>> {
    let mail = read_mail()?;
    let mut msg = Message::parse(&mail);

    // Handle -f (set sender) and -o (override fake From_)
    if let Some(ref sender) = args.from {
        let dominated = args.override_from || msg.from_line().is_none();
        if dominated {
            msg.set_envelope_sender(sender);
        }
    }

    // -d: delivery mode - deliver directly to recipient
    if let Some(ref recip) = args.deliver {
        deliver_to_recipient(&penv, &msg, recip)?;
        return Ok(None);
    }

    let argv = if args.mailfilter {
        collect_trailing_args(&args.rest)
    } else {
        args.args
    };
    let ctx = SubstCtx {
        argv,
        ..Default::default()
    };

    let mut engine = Engine::new(RealEnv, ctx);

    if getenv("VERBOSE").is_some_and(|v| v == "on" || v == "yes" || v == "1") {
        engine.set_verbose(true);
    }

    let mut delivered = false;

    // Process assignments and find rcfiles
    let (assignments, rcfiles) = parse_rest(&args.rest);

    for (name, value) in &assignments {
        engine.set_var(name, value);
    }

    // Process /etc/procmailrc if not in preserve mode (-p), no rcfile on
    // command line, and not in delivery mode (-d).
    let has_cmdline_rcfile = !rcfiles.is_empty();
    if !args.preserve
        && !has_cmdline_rcfile
        && args.deliver.is_none()
        && let Some(Outcome::Delivered(_)) =
            process_etcrc(&mut engine, &mut msg)?
    {
        return Ok(exit_code(&engine));
    }

    // Find user rcfile to use
    let rcfile = find_rcfile(&rcfiles, &penv, args.mailfilter)?;

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
        deliver_default(&engine, &penv, &msg)?;
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
) -> Result<Option<Outcome>, Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
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
            "system rcfile is world-writable: {}",
            path.display()
        )
        .into());
    }

    Ok(())
}

/// Delivery mode: deliver to named recipient's mailbox.
fn deliver_to_recipient(
    env: &ProcEnv, msg: &Message, recip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate recipient name to prevent path traversal
    if recip.contains('/')
        || recip.contains('\0')
        || recip == ".."
        || recip == "."
    {
        return Err("Invalid recipient name".into());
    }

    // Look up recipient in passwd database
    let Some(user) = User::from_name(recip)? else {
        return Err(format!("Unknown user: {recip}").into());
    };

    // Check privileges: must be root or same user
    let euid = nix::unistd::geteuid();
    if !euid.is_root() && euid != user.uid {
        return Err("Insufficient privileges".into());
    }

    let dest = format!("{}/{}", MAIL_SPOOL, recip);
    let sender = msg.envelope_sender().unwrap_or(&env.logname);
    corpmail::delivery::mbox(
        Path::new(&dest),
        msg,
        sender,
        Default::default(),
    )?;
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

fn getenv(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn setenv(name: &str, value: &str) {
    // SAFETY: called only from init_env before any threads spawn
    unsafe { std::env::set_var(name, value) }
}

/// Initialize process environment. Must be called before any threads spawn.
fn init_env(args: &Args) -> Result<ProcEnv, Box<dyn std::error::Error>> {
    let penv = build_env(args)?;

    // Set user variable assignments into environment
    let (assignments, _) = parse_rest(&args.rest);
    for (name, value) in assignments {
        setenv(&name, &value);
    }

    Ok(penv)
}

fn build_env(args: &Args) -> Result<ProcEnv, Box<dyn std::error::Error>> {
    let mut e = ProcEnv::default();

    // Get user info
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

    // Set hostname
    if let Ok(name) = nix::unistd::gethostname() {
        e.host = name.to_string_lossy().into_owned();
    }

    // Apply to process environment
    if !args.preserve {
        setenv("HOME", &e.home);
        setenv("LOGNAME", &e.logname);
        setenv("SHELL", &e.shell);
        setenv(VAR_PATH, DEF_PATH);
    }
    setenv("MAILDIR", &e.maildir);
    setenv("ORGMAIL", &e.orgmail);
    setenv("DEFAULT", &e.orgmail);
    setenv("HOST", &e.host);
    setenv(VAR_SENDMAILFLAGS, DEF_SENDMAILFLAGS);
    setenv(VAR_PROCMAIL_VERSION, VERSION);
    setenv(VAR_USER_SHELL, &e.shell);
    setenv(VAR_NORESRETRY, &DEF_NORESRETRY.to_string());
    setenv(VAR_SUSPEND, &DEF_SUSPEND.to_string());
    setenv(VAR_LOGABSTRACT, &DEF_LOGABSTRACT.to_string());

    Ok(e)
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

fn collect_trailing_args(rest: &[String]) -> Vec<String> {
    let mut args = Vec::new();
    let mut past_rcfile = false;
    for arg in rest {
        if past_rcfile {
            args.push(arg.clone());
        } else if !is_assignment(arg) {
            past_rcfile = true;
        }
    }
    args
}

fn is_assignment(arg: &str) -> bool {
    if let Some(eq) = arg.find('=') {
        is_var_name(&arg[..eq])
    } else {
        false
    }
}

fn find_rcfile(
    files: &[String], env: &ProcEnv, mailfilter: bool,
) -> Result<Option<ValidatedRcfile>, Box<dyn std::error::Error>> {
    let uid = nix::unistd::getuid().as_raw();

    // Command line rcfiles
    for f in files {
        let path = resolve_rcpath(f, env, mailfilter);
        if let Some(rc) = open_and_check(&path, uid, false)? {
            return Ok(Some(rc));
        }
    }

    // Default: ~/.procmailrc
    if files.is_empty() && !mailfilter {
        let default = PathBuf::from(&env.home).join(".procmailrc");
        if let Some(rc) = open_and_check(&default, uid, true)? {
            return Ok(Some(rc));
        }
    }

    if mailfilter && files.is_empty() {
        return Err("Missing rcfile".into());
    }

    Ok(None)
}

/// Open file and check security on the open handle to prevent TOCTOU.
fn open_and_check(
    path: &Path, uid: u32, is_default: bool,
) -> Result<Option<ValidatedRcfile>, Box<dyn std::error::Error>> {
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
    check_rcfile_security(&file, path, uid, is_default)?;
    Ok(Some(ValidatedRcfile {
        path: path.to_path_buf(),
        file,
    }))
}

/// Check that an rcfile has safe permissions (using fstat on open handle).
fn check_rcfile_security(
    file: &File, path: &Path, uid: u32, is_default: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let meta = file.metadata()?;
    let mode = meta.mode();
    let owner = meta.uid();

    // Must be owned by user or root
    if owner != uid && owner != ROOT_UID {
        return Err(format!(
            "rcfile not owned by user or root: {}",
            path.display()
        )
        .into());
    }

    // Must not be world writable
    if mode & 0o002 != 0 {
        return Err(
            format!("rcfile is world-writable: {}", path.display()).into()
        );
    }

    // Default rcfile must not be group writable
    if is_default && mode & 0o020 != 0 {
        return Err(
            format!("rcfile is group-writable: {}", path.display()).into()
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
fn check_dir_security(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let meta = fs::metadata(path)?;
    let mode = meta.mode();

    // World writable without sticky bit is unsafe
    if mode & 0o002 != 0 && mode & 0o1000 == 0 {
        return Err(
            format!("directory is world-writable: {}", path.display()).into()
        );
    }

    Ok(())
}

fn resolve_rcpath(path: &str, env: &ProcEnv, mailfilter: bool) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || path.starts_with("./") || mailfilter {
        p.to_path_buf()
    } else {
        PathBuf::from(&env.home).join(path)
    }
}

fn deliver_default<E>(
    engine: &Engine<E>, penv: &ProcEnv, msg: &Message,
) -> Result<(), Box<dyn std::error::Error>>
where
    E: Env,
{
    let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");

    for name in ["DEFAULT", "ORGMAIL"] {
        let path = engine
            .get_var(name)
            .map(|v| v.into_owned())
            .unwrap_or_else(|| penv.orgmail.clone());
        if !path.is_empty() {
            corpmail::delivery::mbox(
                Path::new(&path),
                msg,
                sender,
                Default::default(),
            )?;
            return Ok(());
        }
    }

    Err("No delivery destination".into())
}

fn print_version() {
    println!("corpmail v{VERSION}");
    println!("Rust translation of procmail");
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
  -d recip    Delivery mode to named recipient
  -m          General mail filter mode

Recipe flags: HBDaAeEfhbcwWir"
    );
}
