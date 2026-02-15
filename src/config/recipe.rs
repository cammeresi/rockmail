use super::{Action, Condition};

#[cfg(test)]
mod tests;

/// All 15 procmail recipe flags.
#[derive(Debug, Clone, Default)]
pub struct Flags {
    /// `H` — grep header (default true).
    pub head: bool,
    /// `B` — grep body.
    pub body: bool,
    /// `D` — case sensitive.
    pub case: bool,
    /// `A` — chain on prior condition match.
    pub chain: bool,
    /// `a` — chain on prior success.
    pub succ: bool,
    /// `E` — else branch.
    pub r#else: bool,
    /// `e` — error handler.
    pub err: bool,
    /// `h` — pass header (default true).
    pub pass_head: bool,
    /// `b` — pass body (default true).
    pub pass_body: bool,
    /// `f` — filter mode.
    pub filter: bool,
    /// `c` — continue after delivery.
    pub copy: bool,
    /// `w` — wait for program.
    pub wait: bool,
    /// `W` — wait quietly.
    pub quiet: bool,
    /// `i` — ignore write errors.
    pub ignore: bool,
    /// `r` — raw mode.
    pub raw: bool,
}

impl Flags {
    /// Default flags: head, pass_head, and pass_body enabled.
    pub fn new() -> Self {
        Self {
            head: true,
            pass_head: true,
            pass_body: true,
            ..Default::default()
        }
    }

    /// Parse flags from the colon line of a recipe.
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
                'E' => f.r#else = true,
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
                _ => eprintln!("unknown recipe flag: {c}"),
            }
        }
        f
    }
}

/// A single procmail recipe.
#[derive(Debug, Clone)]
pub struct Recipe {
    /// Recipe flags from the `:0` line.
    pub flags: Flags,
    /// Explicit lockfile from the `:0` line (`:` suffix).
    pub lockfile: Option<String>,
    /// Zero or more condition lines (`*` prefix).
    pub conds: Vec<Condition>,
    /// Delivery action (folder, pipe, forward, or nested block).
    pub action: Action,
}

impl Recipe {
    /// Construct a recipe from its parts.
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

    /// Returns true if this is a delivering recipe (writes to
    /// file/forwards/pipes without capture)
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
    /// Variable assignment (`NAME=value`).
    Assign {
        /// Variable name.
        name: String,
        /// Assigned value (after `=`).
        value: String,
    },
    /// A recipe block.
    Recipe(Recipe),
    /// Include an rcfile (INCLUDERC assignment).
    Include(String),
    /// Switch to a different rcfile (SWITCHRC assignment).
    Switch(String),
}
