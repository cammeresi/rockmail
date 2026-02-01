use std::path::PathBuf;

use tempfile::TempDir;

use crate::config::{Action, Condition, Flags, Item, Recipe};
use crate::mail::Message;
use crate::variables::{MockEnv, SubstCtx};

use super::{Engine, Outcome};

struct Test {
    tmp: TempDir,
    engine: Engine<MockEnv>,
    msg: Message,
}

impl Test {
    fn new() -> Self {
        Self::with_msg("Subject: test\n\nHello")
    }

    fn with_msg(text: &str) -> Self {
        Self {
            tmp: TempDir::new().unwrap(),
            engine: Engine::new(MockEnv::new(), SubstCtx::default()),
            msg: Message::parse(text.as_bytes()),
        }
    }

    fn folder(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }

    fn maildir(&self, name: &str) -> String {
        format!("{}/", self.folder(name).display())
    }

    fn process(&mut self, items: &[Item]) -> Outcome {
        self.engine.process(items, &mut self.msg).unwrap()
    }
}

fn regex_recipe(pattern: &str, folder: &str) -> Item {
    Item::Recipe(Recipe {
        flags: Flags::new(),
        lockfile: None,
        conds: vec![Condition::Regex {
            pattern: pattern.to_string(),
            negate: false,
        }],
        action: Action::Folder(PathBuf::from(folder)),
    })
}

#[test]
fn no_recipes_returns_default() {
    let mut t = Test::new();
    assert_eq!(t.process(&[]), Outcome::Default);
}

#[test]
fn matching_regex_delivers() {
    let mut t = Test::new();
    let items = vec![regex_recipe("Subject:", &t.maildir("inbox"))];
    assert!(
        matches!(t.process(&items), Outcome::Delivered(p) if p.contains("inbox"))
    );
}

#[test]
fn non_matching_regex_skips() {
    let mut t = Test::new();
    let items = vec![regex_recipe("X-Spam:", &t.maildir("spam"))];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn negated_regex() {
    let mut t = Test::new();
    let items = vec![Item::Recipe(Recipe {
        flags: Flags::new(),
        lockfile: None,
        conds: vec![Condition::Regex {
            pattern: "X-Spam:".to_string(),
            negate: true,
        }],
        action: Action::Folder(PathBuf::from(t.maildir("inbox"))),
    })];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn size_condition_less() {
    let mut t = Test::new();
    let items = vec![Item::Recipe(Recipe {
        flags: Flags::new(),
        lockfile: None,
        conds: vec![Condition::Size {
            op: std::cmp::Ordering::Less,
            bytes: 1000,
        }],
        action: Action::Folder(PathBuf::from(t.maildir("small"))),
    })];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn size_condition_greater_fails() {
    let mut t = Test::new();
    let items = vec![Item::Recipe(Recipe {
        flags: Flags::new(),
        lockfile: None,
        conds: vec![Condition::Size {
            op: std::cmp::Ordering::Greater,
            bytes: 1000,
        }],
        action: Action::Folder(PathBuf::from(t.maildir("large"))),
    })];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn variable_assignment() {
    let mut t = Test::new();
    let items = vec![
        Item::Assign {
            name: "FOO".to_string(),
            value: "bar".to_string(),
        },
        Item::Recipe(Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Variable {
                name: "FOO".to_string(),
                pattern: "bar".to_string(),
            }],
            action: Action::Folder(PathBuf::from(t.maildir("matched"))),
        }),
    ];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn chain_a_flag_skips_when_prev_false() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.chain = true;

    let items = vec![
        regex_recipe("X-NotPresent:", &t.maildir("first")),
        Item::Recipe(Recipe {
            flags,
            lockfile: None,
            conds: vec![],
            action: Action::Folder(PathBuf::from(t.maildir("second"))),
        }),
    ];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn else_e_flag_runs_when_prev_false() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.else_ = true;

    let items = vec![
        regex_recipe("X-NotPresent:", &t.maildir("first")),
        Item::Recipe(Recipe {
            flags,
            lockfile: None,
            conds: vec![],
            action: Action::Folder(PathBuf::from(t.maildir("else"))),
        }),
    ];
    assert!(
        matches!(t.process(&items), Outcome::Delivered(p) if p.contains("else"))
    );
}

#[test]
fn body_flag_greps_body() {
    let mut t = Test::with_msg("Subject: test\n\nThis is the body");
    let mut flags = Flags::new();
    flags.head = false;
    flags.body = true;

    let items = vec![Item::Recipe(Recipe {
        flags,
        lockfile: None,
        conds: vec![Condition::Regex {
            pattern: "body".to_string(),
            negate: false,
        }],
        action: Action::Folder(PathBuf::from(t.maildir("body"))),
    })];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn copy_flag_continues() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.copy = true;

    let items = vec![
        Item::Recipe(Recipe {
            flags,
            lockfile: None,
            conds: vec![],
            action: Action::Folder(PathBuf::from(t.maildir("first"))),
        }),
        Item::Recipe(Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![],
            action: Action::Folder(PathBuf::from(t.maildir("second"))),
        }),
    ];
    assert!(
        matches!(t.process(&items), Outcome::Delivered(p) if p.contains("second"))
    );
}

#[test]
fn nested_block() {
    let mut t = Test::new();
    let items = vec![Item::Recipe(Recipe {
        flags: Flags::new(),
        lockfile: None,
        conds: vec![Condition::Regex {
            pattern: "Subject:".to_string(),
            negate: false,
        }],
        action: Action::Nested(vec![Item::Recipe(Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![],
            action: Action::Folder(PathBuf::from(t.maildir("inner"))),
        })]),
    })];
    assert!(
        matches!(t.process(&items), Outcome::Delivered(p) if p.contains("inner"))
    );
}
