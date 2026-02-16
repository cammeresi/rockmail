use std::{env, fs, process};

use rockmail::config::dump;

#[cfg(test)]
mod tests;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <procmailrc>", args[0]);
        process::exit(1);
    }

    let path = &args[1];
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            process::exit(1);
        }
    };

    if let Err(e) = dump::run(&content, path) {
        eprintln!("Parse error: {e}");
        process::exit(1);
    }
}
