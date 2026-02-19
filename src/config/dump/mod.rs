//! Parse and pretty-print an rcfile.

use std::cmp::Ordering;

use super::parser::ParseError;
use super::{Action, Condition, Flags, Grep, Item, Recipe, Weight, parse};

fn fmt_flags(f: &Flags) -> String {
    let mut p = Vec::new();

    match f.grep {
        Grep::Full => p.push("HB (grep header+body)"),
        Grep::Headers => p.push("H (grep header)"),
        Grep::Body => p.push("B (grep body)"),
    }

    if f.case {
        p.push("D (case-sensitive)");
    }
    if f.chain {
        p.push("A (chain on match)");
    }
    if f.succ {
        p.push("a (chain on success)");
    }
    if f.r#else {
        p.push("E (else branch)");
    }
    if f.err {
        p.push("e (error handler)");
    }
    if !f.pass_head {
        p.push("!h (no header)");
    }
    if !f.pass_body {
        p.push("!b (no body)");
    }
    if f.filter {
        p.push("f (filter)");
    }
    if f.copy {
        p.push("c (copy)");
    }
    if f.wait && f.quiet {
        p.push("W (wait quietly)");
    } else if f.wait {
        p.push("w (wait)");
    }
    if f.ignore {
        p.push("i (ignore errors)");
    }
    if f.raw {
        p.push("r (raw)");
    }

    p.join(", ")
}

fn fmt_weight(w: Option<Weight>) -> String {
    w.map_or(String::new(), |w| format!("{}^{} ", w.w, w.x))
}

fn fmt_cond(c: &Condition) -> String {
    match c {
        Condition::Regex {
            pattern,
            negate,
            weight,
        } => {
            let pre = fmt_weight(*weight);
            if *negate {
                format!("{pre}NOT regex {pattern:?}")
            } else {
                format!("{pre}regex {pattern:?}")
            }
        }
        Condition::Size {
            op,
            bytes,
            negate,
            weight,
        } => {
            let pre = fmt_weight(*weight);
            let neg = if *negate { "!" } else { "" };
            let cmp = match op {
                Ordering::Less => "<",
                Ordering::Greater => ">",
                Ordering::Equal => "=",
            };
            format!("{pre}{neg}size {cmp} {bytes} bytes")
        }
        Condition::Shell {
            cmd,
            negate,
            weight,
        } => {
            let neg = if *negate { "!" } else { "" };
            format!("{}{}shell {:?}", fmt_weight(*weight), neg, cmd)
        }
        Condition::Variable {
            name,
            pattern,
            weight,
        } => {
            format!("{}${} matches {:?}", fmt_weight(*weight), name, pattern)
        }
        Condition::Subst { inner, negate } => {
            let s = fmt_cond(inner);
            if *negate {
                format!("NOT subst({s})")
            } else {
                format!("subst({s})")
            }
        }
    }
}

fn fmt_action(a: &Action, depth: usize) -> String {
    match a {
        Action::Folder(paths) => {
            let s: Vec<_> =
                paths.iter().map(|p| p.display().to_string()).collect();
            let label = s.join(" ");
            if s[0].ends_with('/') {
                format!("deliver to Maildir {label}")
            } else {
                format!("deliver to {label}")
            }
        }
        Action::Pipe { cmd, capture } => {
            if let Some(var) = capture {
                format!("pipe to {cmd:?}, capture to ${var}")
            } else {
                format!("pipe to {cmd:?}")
            }
        }
        Action::Forward(addrs) => {
            format!("forward to {}", addrs.join(", "))
        }
        Action::Nested(items) => {
            let mut out = String::from("nested block:\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&fmt_item_str(item, i + 1, depth + 1));
            }
            out
        }
        Action::DupeCheck { maxlen, cache } => {
            format!("@D {maxlen} {cache}")
        }
    }
}

fn fmt_recipe(out: &mut String, r: &Recipe, depth: usize) {
    let ind = "  ".repeat(depth);
    let flags = fmt_flags(&r.flags);
    if !flags.is_empty() {
        out.push_str(&format!("{ind}Flags: {flags}\n"));
    }
    if let Some(ref lock) = r.lockfile {
        if lock.is_empty() {
            out.push_str(&format!("{ind}Lock: (auto)\n"));
        } else {
            out.push_str(&format!("{ind}Lock: {lock}\n"));
        }
    }
    out.push_str(&format!("{ind}Delivering: {}\n", r.is_delivering()));
    if !r.conds.is_empty() {
        out.push_str(&format!("{ind}Conditions:\n"));
        for (i, c) in r.conds.iter().enumerate() {
            out.push_str(&format!("{ind}  {}. {}\n", i + 1, fmt_cond(c)));
        }
    }
    out.push_str(&format!("{ind}Action: {}\n", fmt_action(&r.action, depth)));
}

fn fmt_subst_flags(global: bool, ci: bool) -> &'static str {
    match (global, ci) {
        (true, true) => "gi",
        (true, false) => "g",
        (false, true) => "i",
        (false, false) => "",
    }
}

fn fmt_item_str(item: &Item, num: usize, depth: usize) -> String {
    let ind = "  ".repeat(depth);
    let mut out = String::new();
    match item {
        Item::Assign { name, value, .. } => {
            if value.is_empty() {
                out.push_str(&format!("{ind}{num:3}. [UNSET] {name}\n"));
            } else {
                out.push_str(&format!(
                    "{ind}{num:3}. [ASSIGN] {name} = {value:?}\n"
                ));
            }
        }
        Item::Recipe { recipe: r, .. } => {
            out.push_str(&format!("{ind}{num:3}. [RECIPE]\n"));
            fmt_recipe(&mut out, r, depth + 1);
        }
        Item::Subst {
            name,
            pattern,
            replace,
            global,
            case_insensitive,
            ..
        } => {
            let f = fmt_subst_flags(*global, *case_insensitive);
            out.push_str(&format!(
                "{ind}{num:3}. [SUBST] {name} =~ s/{pattern}/{replace}/{f}\n"
            ));
        }
        Item::HeaderOp { op, .. } => {
            out.push_str(&format!("{ind}{num:3}. [HEADEROP] {op:?}\n"));
        }
        Item::Include { path, .. } => {
            out.push_str(&format!("{ind}{num:3}. [INCLUDERC] {path:?}\n"));
        }
        Item::Switch { path, .. } => {
            if path.is_empty() {
                out.push_str(&format!("{ind}{num:3}. [SWITCHRC] (abort)\n"));
            } else {
                out.push_str(&format!("{ind}{num:3}. [SWITCHRC] {path:?}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;

/// Parse an rcfile and print a human-readable dump of every item.
pub fn run(content: &str, path: &str) -> Result<Vec<Item>, ParseError> {
    let items = parse(content, path)?;
    println!("Parsed {} items from {}\n", items.len(), path);
    for (i, item) in items.iter().enumerate() {
        let s = fmt_item_str(item, i + 1, 0);
        println!("{s}");
    }
    Ok(items)
}
