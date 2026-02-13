use std::cmp::Ordering;
use std::env;
use std::fs;
use std::process;

use rockmail::config::{Action, Condition, Flags, Item, Recipe, Weight, parse};

fn format_flags(f: &Flags) -> String {
    let mut parts = Vec::new();

    // Grep area
    if f.head && f.body {
        parts.push("HB (grep header+body)");
    } else if f.head {
        parts.push("H (grep header)");
    } else if f.body {
        parts.push("B (grep body)");
    }

    // Case sensitivity
    if f.case {
        parts.push("D (case-sensitive)");
    }

    // Chaining
    if f.chain {
        parts.push("A (chain on match)");
    }
    if f.succ {
        parts.push("a (chain on success)");
    }
    if f.r#else {
        parts.push("E (else branch)");
    }
    if f.err {
        parts.push("e (error handler)");
    }

    // Pass-through
    if !f.pass_head {
        parts.push("!h (no header)");
    }
    if !f.pass_body {
        parts.push("!b (no body)");
    }

    // Modes
    if f.filter {
        parts.push("f (filter)");
    }
    if f.copy {
        parts.push("c (copy)");
    }
    if f.wait && f.quiet {
        parts.push("W (wait quietly)");
    } else if f.wait {
        parts.push("w (wait)");
    }
    if f.ignore {
        parts.push("i (ignore errors)");
    }
    if f.raw {
        parts.push("r (raw)");
    }

    parts.join(", ")
}

fn format_weight(w: Option<Weight>) -> String {
    w.map_or(String::new(), |w| format!("{}^{} ", w.w, w.x))
}

fn format_condition(c: &Condition) -> String {
    match c {
        Condition::Regex {
            pattern,
            negate,
            weight,
        } => {
            let prefix = format_weight(*weight);
            if *negate {
                format!("{}NOT regex {:?}", prefix, pattern)
            } else {
                format!("{}regex {:?}", prefix, pattern)
            }
        }
        Condition::Size { op, bytes, weight } => {
            let prefix = format_weight(*weight);
            let cmp = match op {
                Ordering::Less => "<",
                Ordering::Greater => ">",
                Ordering::Equal => "=",
            };
            format!("{}size {} {} bytes", prefix, cmp, bytes)
        }
        Condition::Shell { cmd, weight } => {
            format!("{}shell {:?}", format_weight(*weight), cmd)
        }
        Condition::Variable {
            name,
            pattern,
            weight,
        } => {
            format!("{}${} matches {:?}", format_weight(*weight), name, pattern)
        }
        Condition::Subst { inner, negate } => {
            let inner_str = format_condition(inner);
            if *negate {
                format!("NOT subst({})", inner_str)
            } else {
                format!("subst({})", inner_str)
            }
        }
    }
}

fn format_action(a: &Action, depth: usize) -> String {
    match a {
        Action::Folder(p) => {
            let p = p.display();
            if p.to_string().ends_with('/') {
                format!("deliver to Maildir {}", p)
            } else {
                format!("deliver to {}", p)
            }
        }
        Action::Pipe { cmd, capture } => {
            if let Some(var) = capture {
                format!("pipe to {:?}, capture to ${}", cmd, var)
            } else {
                format!("pipe to {:?}", cmd)
            }
        }
        Action::Forward(addrs) => {
            format!("forward to {}", addrs.join(", "))
        }
        Action::Nested(items) => {
            let mut out = String::from("nested block:\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&format_nested_item(item, i + 1, depth + 1));
            }
            out
        }
    }
}

fn format_nested_item(item: &Item, num: usize, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    match item {
        Item::Assign { name, value } => {
            if value.is_empty() {
                format!("{}{:3}. [UNSET] {}\n", indent, num, name)
            } else {
                format!(
                    "{}{:3}. [ASSIGN] {} = {:?}\n",
                    indent, num, name, value
                )
            }
        }
        Item::Recipe(r) => {
            let mut out = format!("{}{:3}. [RECIPE]\n", indent, num);
            let inner_indent = "  ".repeat(depth + 1);

            let flags = format_flags(&r.flags);
            if !flags.is_empty() {
                out.push_str(&format!("{}Flags: {}\n", inner_indent, flags));
            }
            if let Some(ref lock) = r.lockfile {
                if lock.is_empty() {
                    out.push_str(&format!("{}Lock: (auto)\n", inner_indent));
                } else {
                    out.push_str(&format!("{}Lock: {}\n", inner_indent, lock));
                }
            }
            out.push_str(&format!(
                "{}Delivering: {}\n",
                inner_indent,
                r.is_delivering()
            ));
            if !r.conds.is_empty() {
                out.push_str(&format!("{}Conditions:\n", inner_indent));
                for (i, c) in r.conds.iter().enumerate() {
                    out.push_str(&format!(
                        "{}  {}. {}\n",
                        inner_indent,
                        i + 1,
                        format_condition(c)
                    ));
                }
            }
            out.push_str(&format!(
                "{}Action: {}\n",
                inner_indent,
                format_action(&r.action, depth + 1)
            ));
            out
        }
        Item::Include(path) => {
            format!("{}{:3}. [INCLUDERC] {:?}\n", indent, num, path)
        }
        Item::Switch(path) => {
            if path.is_empty() {
                format!("{}{:3}. [SWITCHRC] (abort)\n", indent, num)
            } else {
                format!("{}{:3}. [SWITCHRC] {:?}\n", indent, num, path)
            }
        }
    }
}

fn print_recipe(r: &Recipe, depth: usize) {
    let indent = "  ".repeat(depth);

    // Flags
    let flags = format_flags(&r.flags);
    if !flags.is_empty() {
        println!("{}Flags: {}", indent, flags);
    }

    // Lockfile
    if let Some(ref lock) = r.lockfile {
        if lock.is_empty() {
            println!("{}Lock: (auto)", indent);
        } else {
            println!("{}Lock: {}", indent, lock);
        }
    }

    // Delivering?
    println!("{}Delivering: {}", indent, r.is_delivering());

    // Conditions
    if !r.conds.is_empty() {
        println!("{}Conditions:", indent);
        for (i, c) in r.conds.iter().enumerate() {
            println!("{}  {}. {}", indent, i + 1, format_condition(c));
        }
    }

    // Action
    println!("{}Action: {}", indent, format_action(&r.action, depth));
}

fn print_item(item: &Item, num: usize, depth: usize) {
    let indent = "  ".repeat(depth);
    match item {
        Item::Assign { name, value } => {
            if value.is_empty() {
                println!("{}{:3}. [UNSET] {}", indent, num, name);
            } else {
                println!(
                    "{}{:3}. [ASSIGN] {} = {:?}",
                    indent, num, name, value
                );
            }
        }
        Item::Recipe(r) => {
            println!("{}{:3}. [RECIPE]", indent, num);
            print_recipe(r, depth + 1);
        }
        Item::Include(path) => {
            println!("{}{:3}. [INCLUDERC] {:?}", indent, num, path);
        }
        Item::Switch(path) => {
            if path.is_empty() {
                println!("{}{:3}. [SWITCHRC] (abort)", indent, num);
            } else {
                println!("{}{:3}. [SWITCHRC] {:?}", indent, num, path);
            }
        }
    }
    println!();
}

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

    match parse(&content) {
        Ok(items) => {
            println!("Parsed {} items from {}\n", items.len(), path);
            for (i, item) in items.iter().enumerate() {
                print_item(item, i + 1, 0);
            }
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    }
}
