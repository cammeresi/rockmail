mod error;

pub use error::*;

use std::process::ExitCode;

pub const EX_OK: u8 = 0;
pub const EX_USAGE: u8 = 64;
pub const EX_TEMPFAIL: u8 = 75;
pub const EX_UNAVAILABLE: u8 = 69;
pub const EX_OSERR: u8 = 71;
pub const EX_CANTCREAT: u8 = 73;

pub fn exit(code: u8) -> ExitCode {
    ExitCode::from(code)
}
