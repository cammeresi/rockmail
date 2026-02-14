//! Formail: mail header manipulation and mailbox splitting.
//!
//! This module provides the core functionality for the formail binary,
//! including header field operations, auto-reply generation, and mailbox
//! splitting.

#[cfg(test)]
mod tests;

mod field;

pub use field::{Field, FieldList, read_headers};
