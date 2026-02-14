use std::borrow::Borrow;

mod builtins;
mod environment;
mod substitution;

pub use builtins::*;
pub use environment::*;
pub use substitution::*;

#[cfg(test)]
mod tests;

/// Parse a string to an integer the way procmail's `renvint()` does.
///
/// First tries decimal parsing.  If that fails, recognizes text aliases:
/// on/y/t/e → 1, off/n/f/d → 0, a → 2.  Falls back to `def`.
pub fn value_as_int<T>(s: T, def: i64) -> i64
where
    T: Borrow<str>,
{
    let s = s.borrow().trim();
    if let Ok(n) = s.parse::<i64>() {
        return n;
    }
    match s.as_bytes().first().copied() {
        Some(b'o' | b'O') => {
            if s[1..].eq_ignore_ascii_case("n") {
                1
            } else if s[1..].eq_ignore_ascii_case("ff") {
                0
            } else {
                def
            }
        }
        Some(b'y' | b'Y' | b't' | b'T' | b'e' | b'E') => 1,
        Some(b'n' | b'N' | b'f' | b'F' | b'd' | b'D') => 0,
        Some(b'a' | b'A') => 2,
        _ => def,
    }
}

/// Check if a variable value is truthy (nonzero via `value_as_int`).
pub fn value_is_true<T>(v: T) -> bool
where
    T: Borrow<str>,
{
    value_as_int(v, 0) != 0
}
