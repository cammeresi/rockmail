mod from_line;
mod message;

pub use from_line::{
    generate, generate_with_time, skip_from_lines, starts_with_from,
};
pub use message::{HeaderIter, Message};
