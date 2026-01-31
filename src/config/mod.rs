mod action;
mod condition;
mod parser;
mod recipe;

pub use action::Action;
pub use condition::Condition;
pub use parser::{parse, ParseError, Parser};
pub use recipe::{Flags, Item, Recipe};

/// Check if a string is a valid variable name (letters, digits, underscore,
/// starting with letter or underscore)
pub(crate) fn is_var_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
