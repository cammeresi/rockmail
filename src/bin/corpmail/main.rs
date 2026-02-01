//! Corpmail binary - autonomous mail processor (procmail replacement).

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use corpmail::config;
use corpmail::mail::Message;
use corpmail::recipe::{Engine, Outcome};
use corpmail::util::{EX_CANTCREAT, EX_TEMPFAIL, EX_USAGE};
use corpmail::variables::{RealEnv, SubstCtx};
use nix::unistd::{Uid, User};

#[cfg(test)]
mod tests;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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

    /// Berkeley format (ignore Content-Length)
    #[arg(short = 'Y', short_alias = 'y')]
    berkeley: bool,

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
    env_logger::init();

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
        return ExitCode::from(EX_USAGE);
    }

    if args.deliver.is_some() && (args.preserve || args.mailfilter) {
        eprintln!("corpmail: Conflicting options");
        return ExitCode::from(EX_USAGE);
    }

    if args.mailfilter && !args.args.is_empty() && !args.rest.is_empty() {
        eprintln!("corpmail: -m with -a: use trailing args instead");
    }

    let fail_code = if args.tempfail {
        EX_TEMPFAIL
    } else {
        EX_CANTCREAT
    };

    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("corpmail: {e}");
            ExitCode::from(fail_code)
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let mail = read_mail()?;
    let mut msg = Message::parse(&mail);

    let argv = if args.mailfilter {
        collect_trailing_args(&args.rest)
    } else {
        args.args.clone()
    };
    let ctx = SubstCtx {
        argv,
        ..Default::default()
    };

    let penv = build_env(&args)?;
    let mut engine = Engine::new(RealEnv, ctx);

    if getenv("VERBOSE").is_some_and(|v| v == "on" || v == "yes" || v == "1") {
        engine.set_verbose(true);
    }

    let mut delivered = false;

    // Process assignments and find rcfiles
    let (assignments, rcfiles) = parse_rest(&args.rest);

    for (name, value) in assignments {
        engine.set_var(&name, &value);
        setenv(&name, &value);
    }

    // Find rcfile to use
    let rcfile = find_rcfile(&rcfiles, &penv, args.mailfilter)?;

    if let Some(path) = rcfile {
        let content = fs::read_to_string(&path)?;
        let items = config::parse(&content)?;
        engine.set_var("_", &path.display().to_string());

        match engine.process(&items, &mut msg)? {
            Outcome::Delivered(_) => delivered = true,
            Outcome::Default | Outcome::Continue => {}
        }
    }

    if !delivered {
        deliver_default(&penv, &msg)?;
    }

    Ok(())
}

fn read_mail() -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf)?;
    Ok(buf)
}

fn getenv(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn setenv(name: &str, value: &str) {
    // SAFETY: single-threaded at startup
    unsafe { std::env::set_var(name, value) }
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
    e.orgmail = format!("/var/mail/{}", e.logname);
    e.default = e.orgmail.clone();

    // Set hostname
    if let Ok(name) = nix::unistd::gethostname() {
        e.host = name.to_string_lossy().into_owned();
    }

    // Apply to process environment
    if !args.preserve {
        setenv("HOME", &e.home);
        setenv("LOGNAME", &e.logname);
        setenv("SHELL", &e.shell);
    }
    setenv("MAILDIR", &e.maildir);
    setenv("ORGMAIL", &e.orgmail);
    setenv("DEFAULT", &e.default);
    setenv("HOST", &e.host);

    Ok(e)
}

#[derive(Default)]
struct ProcEnv {
    logname: String,
    home: String,
    shell: String,
    maildir: String,
    orgmail: String,
    default: String,
    host: String,
}

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

fn is_var_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn collect_trailing_args(rest: &[String]) -> Vec<String> {
    let mut args = Vec::new();
    let mut past_rcfile = false;
    for arg in rest {
        if past_rcfile {
            args.push(arg.clone());
        } else if !arg.contains('=') {
            past_rcfile = true;
        }
    }
    args
}

fn find_rcfile(
    files: &[String], env: &ProcEnv, mailfilter: bool,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    // Command line rcfiles
    for f in files {
        let path = resolve_rcpath(f, env, mailfilter);
        if path.exists() {
            return Ok(Some(path));
        }
    }

    // Default: ~/.procmailrc
    if files.is_empty() && !mailfilter {
        let default = PathBuf::from(&env.home).join(".procmailrc");
        if default.exists() {
            return Ok(Some(default));
        }
    }

    if mailfilter && files.is_empty() {
        return Err("Missing rcfile".into());
    }

    Ok(None)
}

fn resolve_rcpath(path: &str, env: &ProcEnv, mailfilter: bool) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || path.starts_with("./") || mailfilter {
        p.to_path_buf()
    } else {
        PathBuf::from(&env.home).join(path)
    }
}

fn deliver_default(
    env: &ProcEnv, msg: &Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let default = getenv("DEFAULT").unwrap_or_else(|| env.default.clone());
    if !default.is_empty() {
        let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");
        corpmail::delivery::mbox(Path::new(&default), msg, sender)?;
        return Ok(());
    }

    let orgmail = getenv("ORGMAIL").unwrap_or_else(|| env.orgmail.clone());
    if !orgmail.is_empty() && orgmail != default {
        let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");
        corpmail::delivery::mbox(Path::new(&orgmail), msg, sender)?;
        return Ok(());
    }

    Err("No delivery destination".into())
}

fn print_version() {
    println!("corpmail v{VERSION}");
    println!("Rust translation of procmail");
}

fn print_help() {
    println!(
        "Usage: corpmail [-ptoy] [-f sender] [parameter=value | rcfile] ...
       corpmail [-toy] [-f sender] [-a arg] ... -d recipient ...
       corpmail [-pty] -m [parameter=value] ... rcfile [arg] ...
       corpmail -v

Options:
  -v          Display version and exit
  -h, -?      Display this help
  -p          Preserve old environment
  -t          Return EX_TEMPFAIL on error
  -f sender   Regenerate From_ line with sender
  -o          Override fake From_ lines
  -Y, -y      Berkeley format (ignore Content-Length)
  -a arg      Set $1, $2, ... (can be repeated)
  -d recip    Delivery mode to named recipient
  -m          General mail filter mode

Recipe flags: HBDaAeEfhbcwWir"
    );
}
