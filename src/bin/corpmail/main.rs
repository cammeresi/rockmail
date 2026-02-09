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

    let env = init_env(&args);

    // SAFETY:
    // If some day there should be thread spawning, threads must not be
    // spawned prior to the preceding environment setup.

    match run(args, env) {
        Ok(code) => code.map_or(ExitCode::SUCCESS, ExitCode::from),
        Err(e) => {
            eprintln!("corpmail: {e}");
            ExitCode::from(fail_code)
        }
    }
}

fn run(args: Args, env: Environment) -> Result<Option<u8>, Box<dyn Error>> {
    let mail = read_mail()?;
    let mut msg = Message::parse(&mail);
    let mut delivered = false;

    if let Some(ref sender) = args.from
        && (args.override_from || msg.from_line().is_none())
    {
        msg.set_envelope_sender(sender);
    }

    let ctx = SubstCtx::new(args.args);
    let mut engine = Engine::new(env, ctx);

    if engine.get_var(VAR_VERBOSE).is_some_and(value_is_true) {
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
    let rcfile = find_rcfile(&rcfiles, &engine)?;

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
        deliver_default(&mut engine, &msg)?;
    }

    Ok(exit_code(&engine))
}

/// Check for EXITCODE variable override.
fn exit_code(engine: &Engine) -> Option<u8> {
    let v = engine.get_var(VAR_EXITCODE)?;
    v.parse::<u8>().ok()
}

/// Process /etc/procmailrc if it exists.
fn process_etcrc(
    engine: &mut Engine, msg: &mut Message,
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

/// Build Environment and set user assignments. Must be called before threads.
fn init_env(args: &Args) -> Environment {
    let mut env = build_env(args);

    let (assignments, _) = parse_rest(&args.rest);
    for (name, value) in assignments {
        env.set(&name, &value);
    }

    env
}

fn build_env(args: &Args) -> Environment {
    let mut env = if args.preserve {
        Environment::from_process()
    } else {
        let mut env = Environment::new();
        // Preserve TZ from process env
        if let Ok(tz) = std::env::var(VAR_TZ) {
            env.set(VAR_TZ, &tz);
        }
        env
    };

    // SAFETY: clear process env for defense in depth (subprocess leaks)
    unsafe {
        std::env::vars().for_each(|(k, _)| std::env::remove_var(k));
    }

    let uid = nix::unistd::getuid();
    let (logname, home, shell) = if let Some(u) = get_user_by_uid(uid.as_raw())
    {
        (u.name, u.home, u.shell)
    } else {
        (format!("#{}", uid), "/".into(), "/bin/sh".into())
    };

    let maildir = format!("{home}/Mail");
    let orgmail = format!("{MAIL_SPOOL}/{logname}");
    let host = nix::unistd::gethostname()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    env.set(VAR_HOME, &home);
    env.set(VAR_LOGNAME, &logname);
    env.set(VAR_SHELL, &shell);
    env.set(VAR_PATH, DEF_PATH);
    env.set(VAR_MAILDIR, &maildir);
    env.set(VAR_ORGMAIL, &orgmail);
    env.set(VAR_DEFAULT, &orgmail);
    env.set(VAR_HOST, &host);
    env.set(VAR_SENDMAILFLAGS, DEF_SENDMAILFLAGS);
    env.set(VAR_PROCMAIL_VERSION, VERSION);
    env.set(VAR_USER_SHELL, &shell);
    env.set(VAR_NORESRETRY, &DEF_NORESRETRY.to_string());
    env.set(VAR_SUSPEND, &DEF_SUSPEND.to_string());
    env.set(VAR_LOGABSTRACT, &DEF_LOGABSTRACT.to_string());

    env
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
    files: &[String], engine: &Engine,
) -> Result<Option<ValidatedRcfile>, Box<dyn Error>> {
    let home = engine.get_var(VAR_HOME).unwrap_or("");

    for f in files {
        let path = resolve_rcpath(f, home);
        if let Some(rc) = open_and_check(&path, false)? {
            return Ok(Some(rc));
        }
    }

    // Default: ~/.procmailrc
    if files.is_empty() {
        let default = PathBuf::from(home).join(".procmailrc");
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

fn resolve_rcpath(path: &str, home: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || path.starts_with("./") {
        p.to_path_buf()
    } else {
        PathBuf::from(home).join(path)
    }
}

fn deliver_default(
    engine: &mut Engine, msg: &Message,
) -> Result<(), Box<dyn Error>> {
    let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");

    for name in [VAR_DEFAULT, VAR_ORGMAIL] {
        let path = engine.get_var(name).unwrap_or("").to_owned();
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
