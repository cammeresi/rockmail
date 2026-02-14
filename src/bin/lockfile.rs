#![cfg(feature = "nfs")]

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use clap::Parser;
use rockmail::locking::{
    MAX_LOCK_SIZE, create_lock, lock_mtime, remove_lock, truncate_lock_path,
};
use rockmail::util::{LockError, exit, now_secs, signals, *};

const DEF_LOCKSLEEP: u64 = 8;
const DEF_SUSPEND: u64 = 16;
const NFS_RETRIES: u32 = 7;

#[derive(Parser)]
#[command(name = "lockfile")]
#[command(about = "Conditional semaphore-file creator")]
#[command(version = rockmail::VERSION, disable_version_flag = true)]
struct Args {
    /// Print version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: (),

    /// Wait this many seconds between locking attempts
    #[arg(short = 'S', long = "sleeptime", default_value_t = DEF_LOCKSLEEP)]
    sleeptime: u64,

    /// Maximum retries before giving up (-1 = forever)
    #[arg(
        short = 'r',
        long = "retries",
        default_value_t = -1,
        allow_hyphen_values = true
    )]
    retries: i64,

    /// Force unlock after this many seconds (0 = disabled)
    #[arg(short = 'l', long = "locktimeout", default_value_t = 0)]
    locktimeout: u64,

    /// Suspend seconds after forced unlock
    #[arg(short = 's', long = "suspend", default_value_t = DEF_SUSPEND)]
    suspend: u64,

    /// Invert the exit code
    #[arg(short = '!', long = "invert", action = clap::ArgAction::Count)]
    invert: u8,

    /// Lock system mailbox
    #[arg(long = "ml", conflicts_with = "unlock_mail")]
    lock_mail: bool,

    /// Unlock system mailbox
    #[arg(long = "mu", conflicts_with = "lock_mail")]
    unlock_mail: bool,

    /// Lockfiles to create
    files: Vec<PathBuf>,
}

fn mailbox_lock() -> Option<PathBuf> {
    let user = env::var("LOGNAME").or_else(|_| env::var("USER")).ok()?;
    let path = format!("/var/mail/{}.lock", user);
    Some(PathBuf::from(path))
}

fn maybe_invert(code: u8, invert: bool) -> ExitCode {
    if invert {
        match code {
            EX_OK => exit(EX_CANTCREAT),
            EX_CANTCREAT => exit(EX_OK),
            other => exit(other),
        }
    } else {
        exit(code)
    }
}

fn cleanup(acquired: &[PathBuf]) {
    for path in acquired {
        if let Err(e) = remove_lock(path) {
            eprintln!(
                "lockfile: warning: cleanup failed for \"{}\": {}",
                path.display(),
                e
            );
        }
    }
}

fn check_signal(path: &Path) -> Result<(), u8> {
    if signals::should_exit() {
        eprintln!(
            "lockfile: Signal received, giving up on \"{}\"",
            path.display()
        );
        Err(EX_TEMPFAIL)
    } else {
        Ok(())
    }
}

fn try_force_unlock(path: &Path, force: u64, suspend: u64) -> bool {
    if force == 0 {
        return false;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.is_dir() || meta.len() > MAX_LOCK_SIZE {
        return false;
    }
    let Some(mtime) = lock_mtime(path) else {
        return false;
    };
    let now = now_secs();
    if now <= mtime || now - mtime <= force {
        return false;
    }
    if remove_lock(path).is_ok() {
        eprintln!("lockfile: Forcing lock on \"{}\"", path.display());
        sleep(Duration::from_secs(suspend));
        true
    } else {
        eprintln!("lockfile: Forced unlock denied on \"{}\"", path.display());
        false
    }
}

fn handle_exists(
    path: &Path, sleepsec: u64, retries: &mut i64, force: u64, suspend: u64,
) -> Result<bool, u8> {
    if try_force_unlock(path, force, suspend) {
        return Ok(true); // retry immediately
    }
    match *retries {
        0 => {
            eprintln!("lockfile: Sorry, giving up on \"{}\"", path.display());
            return Err(EX_CANTCREAT);
        }
        n if n > 0 => *retries -= 1,
        _ => {}
    }
    if sleepsec > 0 {
        sleep(Duration::from_secs(sleepsec));
    }
    Ok(false)
}

fn handle_nfs_error(
    path: &Path, sleepsec: u64, nfs_retry: &mut u32,
) -> Result<(), u8> {
    *nfs_retry = nfs_retry.saturating_sub(1);
    if *nfs_retry == 0 {
        eprintln!("lockfile: Retries exhausted on \"{}\"", path.display());
        return Err(EX_UNAVAILABLE);
    }
    if sleepsec > 0 {
        sleep(Duration::from_secs(sleepsec));
    }
    Ok(())
}

fn try_lock(
    path: &Path, sleepsec: u64, retries: &mut i64, force: u64, suspend: u64,
    acquired: &mut Vec<PathBuf>,
) -> Result<(), u8> {
    let mut nfs_retry = NFS_RETRIES;
    let mut owned = String::new();

    loop {
        let p = if owned.is_empty() {
            path
        } else {
            Path::new(&owned)
        };
        check_signal(p)?;

        match create_lock(p) {
            Ok(()) => {
                acquired.push(p.to_path_buf());
                return Ok(());
            }
            Err(LockError::Exists) => {
                handle_exists(p, sleepsec, retries, force, suspend)?;
            }
            Err(LockError::TooLong) => {
                if owned.is_empty() {
                    owned = path.to_string_lossy().into_owned();
                }
                if !truncate_lock_path(&mut owned) {
                    eprintln!(
                        "lockfile: Filename too long: \"{}\"",
                        path.display()
                    );
                    return Err(EX_UNAVAILABLE);
                }
            }
            Err(LockError::Unavailable | LockError::Io(_)) => {
                handle_nfs_error(p, sleepsec, &mut nfs_retry)?;
            }
        }
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    signals::setup();

    let invert = args.invert % 2 == 1;
    let mut retries = args.retries;
    let mut acquired: Vec<PathBuf> = Vec::with_capacity(args.files.len());
    let mut retval = EX_OK;

    if args.unlock_mail {
        if let Some(mb) = mailbox_lock() {
            if let Err(e) = remove_lock(&mb) {
                eprintln!("lockfile: Can't unlock \"{}\": {}", mb.display(), e);
            }
        } else {
            eprintln!("lockfile: Can't determine your mailbox");
            return exit(EX_OSERR);
        }
        return maybe_invert(EX_OK, invert);
    }

    if args.lock_mail {
        if let Some(mb) = mailbox_lock() {
            match try_lock(
                &mb,
                args.sleeptime,
                &mut retries,
                args.locktimeout,
                args.suspend,
                &mut acquired,
            ) {
                Ok(()) => {}
                Err(code) => {
                    cleanup(&acquired);
                    return maybe_invert(code, invert);
                }
            }
        } else {
            eprintln!("lockfile: Can't determine your mailbox");
            return exit(EX_OSERR);
        }
    }

    if args.files.is_empty() && !args.lock_mail {
        eprintln!("lockfile: No files specified");
        return exit(EX_USAGE);
    }

    for path in &args.files {
        match try_lock(
            path,
            args.sleeptime,
            &mut retries,
            args.locktimeout,
            args.suspend,
            &mut acquired,
        ) {
            Ok(()) => {}
            Err(code) => {
                retval = code;
                cleanup(&acquired);
                return maybe_invert(retval, invert);
            }
        }
    }

    maybe_invert(retval, invert)
}
