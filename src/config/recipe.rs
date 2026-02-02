use super::{Action, Condition};

#[cfg(test)]
mod tests;

/// All 15 procmail recipe flags
#[derive(Debug, Clone, Default)]
pub struct Flags {
    pub head: bool,  // H - grep header (default true)
    pub body: bool,  // B - grep body
    pub case: bool,  // D - case sensitive
    pub chain: bool, // A - chain on prior condition match
    pub succ: bool,  // a - chain on prior success
    pub else_: bool, // E - else branch
    pub err: bool,   // e - error handler

    pub pass_head: bool, // h - pass header (default true)
    pub pass_body: bool, // b - pass body (default true)

    pub filter: bool, // f - filter mode
    pub copy: bool,   // c - continue after delivery
    pub wait: bool,   // w - wait for program
    pub quiet: bool,  // W - wait quietly
    pub ignore: bool, // i - ignore write errors
    pub raw: bool,    // r - raw mode
}

impl Flags {
    pub fn new() -> Self {
        Self {
            head: true,
            pass_head: true,
            pass_body: true,
            ..Default::default()
        }
    }

    pub fn parse(s: &str) -> Self {
        let mut f = Flags::new();
        // If any of H/B specified, reset defaults
        let has_hb = s.chars().any(|c| c == 'H' || c == 'B');
        if has_hb {
            f.head = false;
        }
        for c in s.chars() {
            match c {
                'H' => f.head = true,
                'B' => f.body = true,
                'D' => f.case = true,
                'A' => f.chain = true,
                'a' => f.succ = true,
                'E' => f.else_ = true,
                'e' => f.err = true,
                'h' => f.pass_head = true,
                'b' => f.pass_body = true,
                'f' => f.filter = true,
                'c' => f.copy = true,
                'w' => f.wait = true,
                'W' => {
                    f.wait = true;
                    f.quiet = true;
                }
                'i' => f.ignore = true,
                'r' => f.raw = true,
                ' ' | '\t' => {}
                _ => log::warn!("unknown recipe flag: {c}"),
            }
        }
        f
    }
}

/// A single procmail recipe
#[derive(Debug, Clone)]
pub struct Recipe {
    pub flags: Flags,
    pub lockfile: Option<String>,
    pub conds: Vec<Condition>,
    pub action: Action,
}

impl Recipe {
    pub fn new(
        flags: Flags, lockfile: Option<String>, conds: Vec<Condition>,
        action: Action,
    ) -> Self {
        Self {
            flags,
            lockfile,
            conds,
            action,
        }
    }

    /// Returns true if this is a delivering recipe (writes to file/forwards/pipes without capture)
    pub fn is_delivering(&self) -> bool {
        match &self.action {
            Action::Folder(_) => true,
            Action::Forward(_) => true,
            Action::Pipe { capture: None, .. } => true,
            Action::Pipe {
                capture: Some(_), ..
            } => false,
            Action::Nested(_) => false,
        }
    }
}

/// An rcfile item: variable assignment, recipe, or include directive.
#[derive(Debug, Clone)]
pub enum Item {
    /// Variable assignment (NAME=value).
    Assign { name: String, value: String },
    /// A recipe block.
    Recipe(Recipe),
    /// Include an rcfile (INCLUDERC assignment).
    Include(String),
    /// Switch to a different rcfile (SWITCHRC assignment).
    Switch(String),
}
