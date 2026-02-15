//! Procmail builtin variable names and their defaults.
#![allow(missing_docs)]

use linker_set::*;
use pastey::paste;

/// A named variable with an optional default value.
pub struct Variable {
    pub name: &'static str,
    pub def: Option<&'static str>,
}

set_declare!(defaults, Variable);

macro_rules! var {
    ($id:ident) => {
        var!(@inner $id, None,);
    };
    ($id:ident, $def:expr) => {
        var!(@inner $id, Some($def), #[set_entry(defaults)]);
    };
    (@inner $id:ident, $def:expr, $(#[$attr:meta])*) => {
        $(#[$attr])*
        pub static $id: Variable = Variable {
            name: stringify!($id),
            def: $def,
        };
        paste! {
            pub const [<VAR_ $id>]: &str = stringify!($id);
        }
    };
}

var!(SHELL, "/bin/sh");
var!(SHELLFLAGS, "-c");
var!(SHELLMETAS, "&|<>~;?*[");
var!(LOCKEXT, ".lock");
var!(MSGPREFIX, "msg.");
var!(MAILDIR, "Mail");
var!(SENDMAIL, "/usr/sbin/sendmail");
var!(SENDMAILFLAGS, "-oi");
var!(PATH, "/usr/local/bin:/usr/bin:/bin");
var!(LOCKSLEEP, "8");
var!(LOCKTIMEOUT, "1024");
var!(TIMEOUT, "960");
var!(NORESRETRY, "4");
var!(SUSPEND, "16");
var!(LOGABSTRACT, "-1");
var!(LINEBUF, "2048");
var!(VERBOSE, "no");
var!(UMASK, "077");

var!(HOME);
var!(LOGNAME);
var!(LASTFOLDER);
var!(MATCH);
var!(DEFAULT);
var!(LOGFILE);
var!(LOCKFILE);
var!(HOST);
var!(ORGMAIL);
var!(DELIVERED);
var!(EXITCODE);
var!(INCLUDERC);
var!(SWITCHRC);
var!(LOG);
var!(TRAP);
var!(PROCMAIL_VERSION);
var!(SHIFT);
var!(PROCMAIL_OVERFLOW);
var!(USER_SHELL);
var!(TZ);

// Standalone constants (non-string types or not variable names)
pub const DEF_LINEBUF: usize = 2048;
pub const MIN_LINEBUF: usize = 128;
pub const DEF_UMASK: u32 = 0o077;
pub const DEV_NULL: &str = "/dev/null";
