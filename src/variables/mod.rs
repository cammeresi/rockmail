use std::borrow::Borrow;

mod builtins;
mod substitution;

pub use builtins::*;
pub use substitution::*;

pub fn value_is_true<T>(v: T) -> bool
where
    T: Borrow<str>,
{
    let v = v.borrow();
    v == "yes" || v == "on" || v == "1"
}
