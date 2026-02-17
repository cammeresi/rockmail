use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use crate::config::{Action, Condition, Flags, HeaderOp, Item, Recipe, Weight};
use crate::mail::Message;
use crate::re::Matcher;
use crate::variables::{Environment, SubstCtx};

use super::{Engine, EngineError, Outcome, score_regex};

struct Test {
    tmp: TempDir,
    engine: Engine,
    msg: Message,
}

impl Test {
    fn new() -> Self {
        Self::with_msg("Subject: test\n\nHello")
    }

    fn with_msg(text: &str) -> Self {
        Self {
            tmp: TempDir::new().unwrap(),
            engine: Engine::new(Environment::new(), SubstCtx::default()),
            msg: Message::parse(text.as_bytes()),
        }
    }

    fn folder(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }

    fn maildir(&self, name: &str) -> String {
        format!("{}/", self.folder(name).display())
    }

    fn try_process(&mut self, items: &[Item]) -> Result<Outcome, EngineError> {
        self.engine.process(items, &mut self.msg)
    }

    fn process(&mut self, items: &[Item]) -> Outcome {
        self.try_process(items).unwrap()
    }
}

fn regex_recipe(pattern: &str, folder: &str) -> Item {
    Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: pattern.to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(folder)]),
        },
        line: 0,
    }
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
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "X-Spam:".to_string(),
                negate: true,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("inbox"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn size_condition_less() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Size {
                op: std::cmp::Ordering::Less,
                bytes: 1000,
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("small"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn size_condition_greater_fails() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Size {
                op: std::cmp::Ordering::Greater,
                bytes: 1000,
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("large"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn variable_assignment() {
    let mut t = Test::new();
    let items = vec![
        Item::Assign {
            name: "FOO".to_string(),
            value: "bar".to_string(),
            line: 0,
        },
        Item::Recipe {
            recipe: Recipe {
                flags: Flags::new(),
                lockfile: None,
                conds: vec![Condition::Variable {
                    name: "FOO".to_string(),
                    pattern: "bar".to_string(),
                    weight: None,
                }],
                action: Action::Folder(vec![PathBuf::from(
                    t.maildir("matched"),
                )]),
            },
            line: 0,
        },
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
        Item::Recipe {
            recipe: Recipe {
                flags,
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(
                    t.maildir("second"),
                )]),
            },
            line: 0,
        },
    ];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn else_flag_runs_when_prev_false() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.r#else = true;

    let items = vec![
        regex_recipe("X-NotPresent:", &t.maildir("first")),
        Item::Recipe {
            recipe: Recipe {
                flags,
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(t.maildir("else"))]),
            },
            line: 0,
        },
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

    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags,
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "body".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("body"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn copy_flag_continues() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.copy = true;

    let items = vec![
        Item::Recipe {
            recipe: Recipe {
                flags,
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(t.maildir("first"))]),
            },
            line: 0,
        },
        Item::Recipe {
            recipe: Recipe {
                flags: Flags::new(),
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(
                    t.maildir("second"),
                )]),
            },
            line: 0,
        },
    ];
    assert!(
        matches!(t.process(&items), Outcome::Delivered(p) if p.contains("second"))
    );
}

#[test]
fn nested_block() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "Subject:".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Nested(vec![Item::Recipe {
                recipe: Recipe {
                    flags: Flags::new(),
                    lockfile: None,
                    conds: vec![],
                    action: Action::Folder(vec![PathBuf::from(
                        t.maildir("inner"),
                    )]),
                },
                line: 0,
            }]),
        },
        line: 0,
    }];
    assert!(
        matches!(t.process(&items), Outcome::Delivered(p) if p.contains("inner"))
    );
}

#[test]
fn invalid_regex_returns_error() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "[invalid".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("inbox"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.try_process(&items), Err(EngineError::Regex(_))));
}

#[test]
fn delivery_to_unwritable_path_returns_error() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![],
            action: Action::Folder(vec![PathBuf::from(
                "/nonexistent/deeply/nested/path/",
            )]),
        },
        line: 0,
    }];
    assert!(matches!(
        t.try_process(&items),
        Err(EngineError::Delivery(_))
    ));
}

#[test]
fn subst_negation_inverts_match() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Subst {
                inner: Box::new(Condition::Regex {
                    pattern: "X-NotPresent:".to_string(),
                    negate: false,
                    weight: None,
                }),
                negate: true,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("negated"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn subst_expands_variables() {
    let mut t = Test::new();
    t.engine.set_var("SENDER", "test");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Subst {
                inner: Box::new(Condition::Regex {
                    pattern: "^Subject:.*$SENDER".to_string(),
                    negate: false,
                    weight: None,
                }),
                negate: false,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("subst"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn regex_without_subst_no_expansion() {
    let mut t = Test::new();
    t.engine.set_var("SENDER", "test");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            // Without $, the literal string "$SENDER" should NOT be expanded
            conds: vec![Condition::Regex {
                pattern: "^Subject:.*$SENDER".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("nosubst"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Default));
}

#[test]
fn weighted_condition_positive_score_matches() {
    let mut t = Test::with_msg("Subject: test test test\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "test".to_string(),
                negate: false,
                weight: Some(Weight { w: 100.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("weighted"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn weighted_condition_zero_matches_fails() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "nomatch".to_string(),
                negate: false,
                weight: Some(Weight { w: 100.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("weighted"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn weighted_negated_match_scores_zero() {
    let mut t = Test::with_msg("Subject: spam spam spam\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "spam".to_string(),
                negate: true,
                weight: Some(Weight { w: 100.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("negated"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn weighted_negated_nonmatch_adds_weight() {
    let mut t = Test::with_msg("Subject: hello\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "spam".to_string(),
                negate: true,
                weight: Some(Weight { w: 100.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("negated"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn weighted_empty_match_tail_sum() {
    let mut t = Test::with_msg("Subject: test\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "^".to_string(),
                negate: false,
                weight: Some(Weight { w: 2.0, x: 0.5 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("tailsum"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn action_folder_expands_variable() {
    let mut t = Test::new();
    let dir = t.maildir("expanded");
    t.engine.set_var("DEST", &dir);
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![],
            action: Action::Folder(vec![PathBuf::from("$DEST")]),
        },
        line: 0,
    }];
    assert!(matches!(
        t.process(&items),
        Outcome::Delivered(p) if p.contains("expanded")
    ));
}

#[test]
fn action_pipe_expands_variable() {
    let mut t = Test::new();
    t.engine.set_var("CMD", "cat");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![],
            action: Action::Pipe {
                cmd: "$CMD".to_string(),
                capture: None,
            },
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
}

#[test]
fn score_underflow_forces_failure() {
    let mut t = Test::with_msg("Subject: test\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "test".to_string(),
                negate: false,
                weight: Some(Weight {
                    w: super::MIN32,
                    x: 1.0,
                }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("fail"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn positive_fractional_score_rounds_to_one() {
    let mut t = Test::with_msg("Subject: test\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "test".to_string(),
                negate: false,
                weight: Some(Weight { w: 0.5, x: 0.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("frac"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
    assert_eq!(t.engine.ctx.last_score, 1);
}

#[test]
fn last_score_set_after_weighted_recipe() {
    let mut t = Test::with_msg("Subject: test test test\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "test".to_string(),
                negate: false,
                weight: Some(Weight { w: 10.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("score"))]),
        },
        line: 0,
    }];
    assert!(matches!(t.process(&items), Outcome::Delivered(_)));
    assert_eq!(t.engine.ctx.last_score, 30);
}

#[test]
fn no_short_circuit_accumulates_score() {
    // A failing non-weighted condition followed by a weighted condition:
    // score should still be accumulated even though the recipe fails.
    let mut t = Test::with_msg("Subject: test test\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![
                Condition::Regex {
                    pattern: "NOMATCH".to_string(),
                    negate: false,
                    weight: None,
                },
                Condition::Regex {
                    pattern: "test".to_string(),
                    negate: false,
                    weight: Some(Weight { w: 10.0, x: 1.0 }),
                },
            ],
            action: Action::Folder(vec![PathBuf::from(t.maildir("fail"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
    // Score was accumulated despite the non-weighted failure
    assert_eq!(t.engine.ctx.last_score, 20);
}

#[test]
fn no_short_circuit_weighted_after_fail() {
    // Weighted condition alone would match, but non-weighted failure
    // prevents delivery.
    let mut t = Test::with_msg("Subject: hello\n\nBody");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![
                Condition::Regex {
                    pattern: "hello".to_string(),
                    negate: false,
                    weight: Some(Weight { w: 5.0, x: 1.0 }),
                },
                Condition::Regex {
                    pattern: "NOPE".to_string(),
                    negate: false,
                    weight: None,
                },
            ],
            action: Action::Folder(vec![PathBuf::from(t.maildir("nope"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

fn score(pat: &str, text: &str, w: f64, x: f64) -> f64 {
    let m = Matcher::new(pat, false).unwrap();
    score_regex(&m, text, Weight { w, x })
}

#[test]
fn score_line_count_simple() {
    assert_eq!(score("^.*$", "One\nTwo\nThree\n", 1.0, 1.0), 4.0);
}

#[test]
fn score_line_count_single() {
    assert_eq!(score("^.*$", "Single line\n", 1.0, 1.0), 2.0);
}

#[test]
fn score_line_count_blank_lines() {
    assert_eq!(score("^.*$", "a\n\nb\n\n", 1.0, 1.0), 5.0);
}

#[test]
fn score_line_count_no_trailing_newline() {
    assert_eq!(score("^.*$", "a\nb", 1.0, 1.0), 2.0);
}

#[test]
fn score_line_count_empty() {
    assert_eq!(score("^.*$", "", 1.0, 1.0), 0.0);
}

#[test]
fn score_line_count_only_newlines() {
    assert_eq!(score("^.*$", "\n\n\n", 1.0, 1.0), 4.0);
}

#[test]
fn subst_replaces_variable() {
    let mut t = Test::new();
    t.engine.set_var("X", "hello world");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "world".into(),
        replace: "rust".into(),
        global: false,
        case_insensitive: false,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("hello rust"));
}

#[test]
fn subst_global() {
    let mut t = Test::new();
    t.engine.set_var("X", "aaa");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "a".into(),
        replace: "b".into(),
        global: true,
        case_insensitive: false,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("bbb"));
}

#[test]
fn subst_case_insensitive() {
    let mut t = Test::new();
    t.engine.set_var("X", "Hello");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "hello".into(),
        replace: "bye".into(),
        global: false,
        case_insensitive: true,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("bye"));
}

#[test]
fn header_op_delete_insert() {
    let mut t = Test::with_msg("Subject: old\nX-Foo: bar\n\nbody");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::DeleteInsert {
            field: "Subject".into(),
            value: "new".into(),
        },
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("Subject").as_deref(), Some("new"));
}

#[test]
fn header_op_add_if_not_absent() {
    let mut t = Test::with_msg("Subject: test\n\nbody");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::AddIfNot {
            field: "Lines".into(),
            value: "5".into(),
        },
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("Lines").as_deref(), Some("5"));
}

#[test]
fn header_op_add_if_not_present() {
    let mut t = Test::with_msg("Subject: test\nLines: 10\n\nbody");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::AddIfNot {
            field: "Lines".into(),
            value: "5".into(),
        },
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("Lines").as_deref(), Some("10"));
}

#[test]
fn header_op_rename_insert() {
    let mut t = Test::with_msg("Subject: old\n\nbody");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::RenameInsert {
            field: "Subject".into(),
            value: "new".into(),
        },
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("Subject").as_deref(), Some("new"));
    assert_eq!(t.msg.get_header("Old-Subject").as_deref(), Some("old"));
}

#[test]
fn header_ops_batched() {
    let mut t = Test::with_msg("Subject: old\nX-Foo: bar\n\nbody");
    let items = vec![
        Item::HeaderOp {
            op: HeaderOp::DeleteInsert {
                field: "Subject".into(),
                value: "new".into(),
            },
            line: 0,
        },
        Item::HeaderOp {
            op: HeaderOp::AddAlways {
                field: "X-Tag".into(),
                value: "yes".into(),
            },
            line: 0,
        },
        Item::HeaderOp {
            op: HeaderOp::AddIfNot {
                field: "X-Foo".into(),
                value: "ignored".into(),
            },
            line: 0,
        },
    ];
    t.process(&items);
    assert_eq!(t.msg.get_header("Subject").as_deref(), Some("new"));
    assert_eq!(t.msg.get_header("X-Tag").as_deref(), Some("yes"));
    assert_eq!(t.msg.get_header("X-Foo").as_deref(), Some("bar"));
}

#[test]
fn header_op_add_always() {
    let mut t = Test::with_msg("Subject: test\n\nbody");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::AddAlways {
            field: "X-Tag".into(),
            value: "first".into(),
        },
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("X-Tag").as_deref(), Some("first"));
}

#[test]
fn header_op_add_always_duplicate() {
    let mut t = Test::with_msg("X-Tag: existing\n\nbody");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::AddAlways {
            field: "X-Tag".into(),
            value: "second".into(),
        },
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("X-Tag").as_deref(), Some("existing"));
    let raw = std::str::from_utf8(t.msg.header()).unwrap();
    assert!(raw.contains("X-Tag: second"), "duplicate header not added");
}

#[test]
fn dupecheck_new_message() {
    let mut t = Test::with_msg("Message-ID: <unique@example>\n\nbody");
    let cache = t.folder("msgid.cache");
    let items = vec![Item::DupeCheck {
        maxlen: "8192".into(),
        cache: cache.to_string_lossy().into(),
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("DUPLICATE"), Some(""));
}

#[test]
fn dupecheck_duplicate() {
    let mut t = Test::with_msg("Message-ID: <dup@example>\n\nbody");
    let cache = t.folder("msgid.cache");
    let path: String = cache.to_string_lossy().into();
    let items = vec![Item::DupeCheck {
        maxlen: "8192".into(),
        cache: path.clone(),
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("DUPLICATE"), Some(""));

    let mut t2 = Test::with_msg("Message-ID: <dup@example>\n\nbody");
    let items2 = vec![Item::DupeCheck {
        maxlen: "8192".into(),
        cache: path,
        line: 0,
    }];
    t2.process(&items2);
    assert_eq!(t2.engine.get_var("DUPLICATE"), Some("yes"));
}

#[test]
fn dupecheck_no_msgid() {
    let mut t = Test::with_msg("Subject: test\n\nbody");
    let cache = t.folder("msgid.cache");
    let items = vec![Item::DupeCheck {
        maxlen: "8192".into(),
        cache: cache.to_string_lossy().into(),
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("DUPLICATE"), Some(""));
}

#[test]
fn recursion_limit() {
    let mut t = Test::new();
    let rc = t.folder("self.rc");
    let text = format!("INCLUDERC={}", rc.display());
    fs::write(&rc, text).unwrap();
    let items = vec![Item::Include {
        path: rc.to_string_lossy().into(),
        line: 0,
    }];
    let err = t.try_process(&items).unwrap_err();
    assert!(matches!(err, EngineError::RecursionLimit));
}

#[test]
fn lock_failure() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: Some("/no/such/dir/test.lock".into()),
            conds: vec![],
            action: Action::Folder(vec![PathBuf::from(t.maildir("inbox"))]),
        },
        line: 0,
    }];
    let err = t.try_process(&items).unwrap_err();
    assert!(matches!(err, EngineError::Lock(_)));
}

#[test]
fn pipe_spawn_failure() {
    let mut t = Test::new();
    t.engine.set_var("SHELL", "/no/such/shell");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![],
            action: Action::Pipe {
                cmd: "true".into(),
                capture: None,
            },
        },
        line: 0,
    }];
    let err = t.try_process(&items).unwrap_err();
    assert!(matches!(err, EngineError::Delivery(_)));
}
