mod action;
mod condition;
pub mod dump;
mod parser;
mod recipe;

pub use action::Action;
pub use condition::{Condition, Weight};
pub use parser::{ParseWarning, parse};
pub use recipe::{Flags, HeaderOp, Item, Recipe};

/// Check if a string is a valid variable name (letters, digits, underscore,
/// starting with letter or underscore)
pub fn is_var_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
