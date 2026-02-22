use super::{Action, Condition};

#[cfg(test)]
mod tests;

/// Which parts of the message to operate on (grep or delivery).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum MailParts {
    /// Headers only.
    Headers,
    /// Body only.
    Body,
    /// Both headers and body.
    #[default]
    Full,
}

/// All 15 procmail recipe flags.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Flags {
    /// `H`/`B` — which message parts to grep.
    pub grep: MailParts,
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
    /// `h`/`b` — which message parts to pass for delivery.
    pub pass: MailParts,
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
    /// Unrecognized flag characters.
    pub unknown: Vec<char>,
}

impl Flags {
    /// Default flags: grep headers, pass full message.
    pub fn new() -> Self {
        Self {
            grep: MailParts::Headers,
            ..Default::default()
        }
    }

    /// Parse flags from the colon line of a recipe.
    pub fn parse(s: &str) -> Self {
        let mut f = Flags::new();
        let (uh, ub) = (s.contains('H'), s.contains('B'));
        f.grep = match (uh, ub) {
            (true, true) => MailParts::Full,
            (false, true) => MailParts::Body,
            _ => MailParts::Headers,
        };
        let (lh, lb) = (s.contains('h'), s.contains('b'));
        if lh || lb {
            f.pass = match (lh, lb) {
                (true, true) => MailParts::Full,
                (true, false) => MailParts::Headers,
                (false, _) => MailParts::Body,
            };
        }
        for c in s.chars() {
            match c {
                'H' | 'B' | 'h' | 'b' => {}
                'D' => f.case = true,
                'A' => f.chain = true,
                'a' => f.succ = true,
                'E' => f.r#else = true,
                'e' => f.err = true,
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
                _ => f.unknown.push(c),
            }
        }
        f
    }
}

/// A single procmail recipe.
#[derive(Debug, Clone, PartialEq)]
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
            Action::Nested(_)
            | Action::DupeCheck { .. }
            | Action::HeaderOp(_) => false,
        }
    }
}

/// Header manipulation operation (rockmail extension).
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum HeaderOp {
    /// `@I` — delete all matching, then insert.
    DeleteInsert { field: String, value: String },
    /// `@i` — rename existing to `Old-`, insert new.
    RenameInsert { field: String, value: String },
    /// `@a` — add only if header not present.
    AddIfNot { field: String, value: String },
    /// `@A` — always add (append).
    AddAlways { field: String, value: String },
}

#[allow(missing_docs)]
impl HeaderOp {
    pub fn field(&self) -> &str {
        match self {
            Self::DeleteInsert { field, .. }
            | Self::RenameInsert { field, .. }
            | Self::AddIfNot { field, .. }
            | Self::AddAlways { field, .. } => field,
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Self::DeleteInsert { value, .. }
            | Self::RenameInsert { value, .. }
            | Self::AddIfNot { value, .. }
            | Self::AddAlways { value, .. } => value,
        }
    }

    /// Parse `@X Header: value` into a `HeaderOp`.
    pub fn parse(s: &str) -> Option<Self> {
        let rest = s.strip_prefix('@')?;
        let mut chars = rest.chars();
        let op = chars.next()?;
        let rest = chars.as_str().trim_start();
        let colon = rest.find(':')?;
        let field = rest[..colon].trim().to_string();
        if field.is_empty() {
            return None;
        }
        let value = rest[colon + 1..].trim().to_string();
        match op {
            'I' => Some(Self::DeleteInsert { field, value }),
            'i' => Some(Self::RenameInsert { field, value }),
            'a' => Some(Self::AddIfNot { field, value }),
            'A' => Some(Self::AddAlways { field, value }),
            _ => None,
        }
    }
}

/// An rcfile item: variable assignment, recipe, or include directive.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum Item {
    Assign {
        name: String,
        value: String,
        line: usize,
    },
    Subst {
        name: String,
        pattern: String,
        replace: String,
        global: bool,
        case_insensitive: bool,
        line: usize,
    },
    Recipe {
        recipe: Recipe,
        line: usize,
    },
    Include {
        path: String,
        line: usize,
    },
    Switch {
        path: String,
        line: usize,
    },
}
