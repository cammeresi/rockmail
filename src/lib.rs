//! Rockmail: a Rust drop-in replacement for Procmail.
#![warn(missing_docs)]

/// Crate version, set at build time.
pub const VERSION: &str = env!("VERSION");

/// Rcfile parsing: recipes, conditions, actions, and variable assignments.
pub mod config;
/// Mail delivery to folders and pipes.
pub mod delivery;
/// Recipe evaluation loop and condition matching.
pub mod engine;
/// Header field manipulation (formail).
pub mod field;
/// Dotlock (NFS-safe) and flock file locking.
pub mod locking;
/// Message parsing, headers, and From_ line handling.
pub mod mail;
/// Regex compiler/matcher with procmail extensions.
pub(crate) mod re;
/// Exit codes, error types, and signal handling.
pub mod util;
/// Builtin variables, environment, and substitution.
pub mod variables;
