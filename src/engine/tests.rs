use std::fs;
use std::path::PathBuf;
use std::str;
use std::time::Duration;

use tempfile::TempDir;

use crate::config::{
    Action, Condition, Flags, Grep, HeaderOp, Item, Recipe, Weight,
};
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

fn delivered(o: Outcome) -> String {
    let Outcome::Delivered(p) = o else {
        panic!("expected Outcome::Delivered, got {o:?}");
    };
    p
}

#[test]
#[should_panic(expected = "expected Outcome::Delivered")]
fn delivered_panics_on_default() {
    delivered(Outcome::Default);
}

fn err_delivery(r: Result<Outcome, EngineError>) {
    let Err(EngineError::Delivery(_)) = r else {
        panic!("expected EngineError::Delivery, got {r:?}");
    };
}

#[test]
#[should_panic(expected = "expected EngineError::Delivery")]
fn err_delivery_panics_on_ok() {
    err_delivery(Ok(Outcome::Default));
}

fn err_lock(r: Result<Outcome, EngineError>) {
    let Err(EngineError::Lock(_)) = r else {
        panic!("expected EngineError::Lock, got {r:?}");
    };
}

#[test]
#[should_panic(expected = "expected EngineError::Lock")]
fn err_lock_panics_on_ok() {
    err_lock(Ok(Outcome::Default));
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

fn shell_recipe(cmd: &str, folder: &str) -> Item {
    Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: cmd.to_string(),
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
    let p = delivered(t.process(&items));
    assert!(p.contains("inbox"));
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
    delivered(t.process(&items));
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
    delivered(t.process(&items));
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
fn size_weighted_negated_flips_ratio() {
    // msg is 20 bytes.  Condition: !1^1 < 100.
    // Procmail: (<)^negate=1^1=0 -> sizecheck/pivot = 20/100 = 0.2
    // score = 1.0 * 0.2 = 0.2, rounds up to last_score=1
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Size {
                op: std::cmp::Ordering::Less,
                bytes: 100,
                negate: true,
                weight: Some(Weight { w: 1.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, 1);
}

#[test]
fn size_weighted_negated_greater() {
    // msg is 20 bytes.  Condition: !10^1 > 5.
    // Procmail: (>)^negate=0^1=1 -> pivot/sizecheck = 5/20 = 0.25
    // score = 10 * 0.25 = 2.5, last_score=2
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Size {
                op: std::cmp::Ordering::Greater,
                bytes: 5,
                negate: true,
                weight: Some(Weight { w: 10.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, 2);
}

#[test]
fn size_weighted_zero_size() {
    // Empty msg (0 bytes).  Condition: 1^1 < 100.
    // Procmail: (<)^0=1 -> pivot/sizecheck, sizecheck=0, pivot>0 -> plusinfty
    let mut t = Test::with_msg("");
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Size {
                op: std::cmp::Ordering::Less,
                bytes: 100,
                negate: false,
                weight: Some(Weight { w: 1.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, super::MAX32 as i64);
}

#[test]
fn size_weighted_zero_pivot() {
    // msg is 20 bytes.  Condition: 1^1 > 0.
    // Procmail: (>)^0=0 -> sizecheck/pivot, pivot=0 -> plusinfty
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Size {
                op: std::cmp::Ordering::Greater,
                bytes: 0,
                negate: false,
                weight: Some(Weight { w: 1.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, super::MAX32 as i64);
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
    delivered(t.process(&items));
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
    let p = delivered(t.process(&items));
    assert!(p.contains("else"));
}

#[test]
fn body_flag_greps_body() {
    let mut t = Test::with_msg("Subject: test\n\nThis is the body");
    let mut flags = Flags::new();
    flags.grep = Grep::Body;

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
    delivered(t.process(&items));
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
    let p = delivered(t.process(&items));
    assert!(p.contains("second"));
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
    let p = delivered(t.process(&items));
    assert!(p.contains("inner"));
}

#[test]
fn invalid_regex_skips_recipe() {
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
    assert_eq!(t.process(&items), Outcome::Default);
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
    err_delivery(t.try_process(&items));
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
    delivered(t.process(&items));
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
    delivered(t.process(&items));
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
    assert_eq!(t.process(&items), Outcome::Default);
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
    delivered(t.process(&items));
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
    delivered(t.process(&items));
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
    delivered(t.process(&items));
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
    let p = delivered(t.process(&items));
    assert!(p.contains("expanded"));
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
    delivered(t.process(&items));
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
    delivered(t.process(&items));
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
    delivered(t.process(&items));
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
fn subst_bad_regex() {
    let mut t = Test::new();
    t.engine.set_var("X", "hello");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "[invalid".into(),
        replace: "gone".into(),
        global: false,
        case_insensitive: false,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("hello"));
}

#[test]
fn subst_expand_pattern() {
    let mut t = Test::new();
    t.engine.set_var("PAT", "world");
    t.engine.set_var("X", "hello world");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "$PAT".into(),
        replace: "rust".into(),
        global: false,
        case_insensitive: false,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("hello rust"));
}

#[test]
fn subst_expand_replace() {
    let mut t = Test::new();
    t.engine.set_var("REP", "rust");
    t.engine.set_var("X", "hello world");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "world".into(),
        replace: "$REP".into(),
        global: false,
        case_insensitive: false,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("hello rust"));
}

#[test]
fn subst_global_case_insensitive() {
    let mut t = Test::new();
    t.engine.set_var("X", "Foo foo FOO");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "foo".into(),
        replace: "bar".into(),
        global: true,
        case_insensitive: true,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("bar bar bar"));
}

#[test]
fn subst_no_match() {
    let mut t = Test::new();
    t.engine.set_var("X", "hello");
    let items = vec![Item::Subst {
        name: "X".into(),
        pattern: "xyz".into(),
        replace: "gone".into(),
        global: false,
        case_insensitive: false,
        line: 0,
    }];
    t.process(&items);
    assert_eq!(t.engine.get_var("X"), Some("hello"));
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
    let hdr = t.msg.header();
    let raw = str::from_utf8(&hdr).unwrap();
    assert!(raw.contains("X-Tag: second"), "duplicate header not added");
}

fn dedup_recipe(cache: &str) -> Vec<Item> {
    vec![Item::Recipe {
        recipe: Recipe::new(
            Flags::new(),
            None,
            vec![],
            Action::DupeCheck {
                maxlen: "8192".into(),
                cache: cache.into(),
            },
        ),
        line: 0,
    }]
}

#[test]
fn dupecheck_new_message() {
    let mut t = Test::with_msg("Message-ID: <unique@example>\n\nbody");
    let cache = t.folder("msgid.cache");
    t.process(&dedup_recipe(&cache.to_string_lossy()));
    assert_eq!(t.engine.get_var("DUPLICATE"), Some(""));
}

#[test]
fn dupecheck_duplicate() {
    let mut t = Test::with_msg("Message-ID: <dup@example>\n\nbody");
    let cache = t.folder("msgid.cache");
    let path: String = cache.to_string_lossy().into();
    t.process(&dedup_recipe(&path));
    assert_eq!(t.engine.get_var("DUPLICATE"), Some(""));

    let mut t2 = Test::with_msg("Message-ID: <dup@example>\n\nbody");
    t2.process(&dedup_recipe(&path));
    assert_eq!(t2.engine.get_var("DUPLICATE"), Some("yes"));
}

#[test]
fn dupecheck_no_msgid() {
    let mut t = Test::with_msg("Subject: test\n\nbody");
    let cache = t.folder("msgid.cache");
    t.process(&dedup_recipe(&cache.to_string_lossy()));
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
    assert_eq!(
        t.try_process(&items).unwrap_err(),
        EngineError::RecursionLimit
    );
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
    err_lock(t.try_process(&items));
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
    err_delivery(t.try_process(&items));
}

#[test]
fn case_sensitive_flag() {
    let mut t = Test::with_msg("Subject: TEST\n\nBody");
    let mut flags = Flags::new();
    flags.case = true;

    // "TEST" should NOT match lowercase "test" with D flag
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags,
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "test".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("case"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn case_sensitive_flag_matches() {
    let mut t = Test::with_msg("Subject: TEST\n\nBody");
    let mut flags = Flags::new();
    flags.case = true;

    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags,
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "TEST".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("case"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
}

#[test]
fn ignore_flag_suppresses_delivery_error() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.ignore = true;

    let items = vec![
        Item::Recipe {
            recipe: Recipe {
                flags,
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(
                    "/nonexistent/deeply/nested/path/",
                )]),
            },
            line: 0,
        },
        // Delivery after the ignored error should still work
        Item::Recipe {
            recipe: Recipe {
                flags: Flags::new(),
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(
                    t.maildir("fallback"),
                )]),
            },
            line: 0,
        },
    ];
    let p = delivered(t.process(&items));
    assert!(p.contains("fallback"));
}

#[test]
fn else_if_skips_when_prev_matched() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.r#else = true;

    let items = vec![
        // First recipe matches
        regex_recipe("Subject:", &t.maildir("first")),
        // E recipe should be skipped because first matched
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
    let p = delivered(t.process(&items));
    assert!(p.contains("first"));
}

#[test]
fn error_flag_runs_on_failed_action() {
    let mut t = Test::new();
    let mut err = Flags::new();
    err.err = true;

    let mut wf = Flags::new();
    wf.wait = true;

    let items = vec![
        // Pipe exits non-zero with `w` flag → action fails (Outcome::Default)
        Item::Recipe {
            recipe: Recipe {
                flags: wf,
                lockfile: None,
                conds: vec![],
                action: Action::Pipe {
                    cmd: "false".into(),
                    capture: None,
                },
            },
            line: 0,
        },
        // `e` recipe runs because prev matched but action failed
        Item::Recipe {
            recipe: Recipe {
                flags: err,
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(t.maildir("err"))]),
            },
            line: 0,
        },
    ];
    let p = delivered(t.process(&items));
    assert!(p.contains("err"));
}

#[test]
fn error_flag_skips_on_success() {
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.err = true;

    let mut copy = Flags::new();
    copy.copy = true;

    let items = vec![
        Item::Recipe {
            recipe: Recipe {
                flags: copy,
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(t.maildir("first"))]),
            },
            line: 0,
        },
        // `e` recipe should be skipped because prev action succeeded
        Item::Recipe {
            recipe: Recipe {
                flags,
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(t.maildir("err"))]),
            },
            line: 0,
        },
    ];
    // Should deliver to "first" via copy, then skip error handler, then
    // fall through to default
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn quiet_flag_suppresses_pipe_error_message() {
    // W (quiet=true, wait=true) should not print error messages.
    // We can't easily test stderr output, but we can verify the outcome
    // is correct: pipe failure returns Default, not an error.
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.wait = true;
    flags.quiet = true;

    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags,
            lockfile: None,
            conds: vec![],
            action: Action::Pipe {
                cmd: "false".into(),
                capture: None,
            },
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn head_and_body_flag_greps_both() {
    let mut t = Test::with_msg("Subject: test\n\nBody has keyword");
    let mut flags = Flags::new();
    flags.grep = Grep::Full;

    // Pattern in body, grepping both H and B
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags,
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "keyword".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("both"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
}

#[test]
fn head_and_body_case_sensitive() {
    let mut t = Test::with_msg("Subject: TEST\n\nBODY");
    let mut flags = Flags::new();
    flags.grep = Grep::Full;
    flags.case = true;

    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags,
            lockfile: None,
            conds: vec![Condition::Regex {
                pattern: "test".to_string(),
                negate: false,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("hbd"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn copy_and_wait() {
    // cw: copy flag + wait flag on a pipe
    let mut t = Test::new();
    let mut flags = Flags::new();
    flags.copy = true;
    flags.wait = true;

    let items = vec![
        Item::Recipe {
            recipe: Recipe {
                flags,
                lockfile: None,
                conds: vec![],
                action: Action::Pipe {
                    cmd: "true".into(),
                    capture: None,
                },
            },
            line: 0,
        },
        Item::Recipe {
            recipe: Recipe {
                flags: Flags::new(),
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(t.maildir("after"))]),
            },
            line: 0,
        },
    ];
    let p = delivered(t.process(&items));
    assert!(p.contains("after"));
}

#[test]
fn filter_replaces_message() {
    let mut t = Test::with_msg("Subject: test\n\nOriginal body");
    let mut flags = Flags::new();
    flags.filter = true;
    flags.wait = true;

    let items = vec![
        Item::Recipe {
            recipe: Recipe {
                flags,
                lockfile: None,
                conds: vec![],
                action: Action::Pipe {
                    cmd: "printf 'X: y\\n\\nfiltered'".into(),
                    capture: None,
                },
            },
            line: 0,
        },
        Item::Recipe {
            recipe: Recipe {
                flags: Flags::new(),
                lockfile: None,
                conds: vec![],
                action: Action::Folder(vec![PathBuf::from(t.maildir("out"))]),
            },
            line: 0,
        },
    ];
    delivered(t.process(&items));
    assert_eq!(t.msg.body(), b"filtered");
}

#[test]
fn backtick_spawn_failure_returns_empty() {
    let mut env = Environment::new();
    env.set("SHELL", "/no/such/shell");
    let r = super::run_backtick(&env, "echo hi", b"", Duration::from_secs(5));
    assert_eq!(r, "");
}

#[test]
fn backtick_captures_stdout() {
    let env = Environment::new();
    let r =
        super::run_backtick(&env, "echo hello", b"", Duration::from_secs(5));
    assert_eq!(r, "hello");
}

#[test]
fn backtick_strips_trailing_newlines() {
    let env = Environment::new();
    let r = super::run_backtick(
        &env,
        "printf 'abc\\n\\n\\n'",
        b"",
        Duration::from_secs(5),
    );
    assert_eq!(r, "abc");
}

#[test]
fn dryrun_log_header_delete_insert() {
    let mut t = Test::with_msg("Subject: old\n\nbody");
    t.engine.set_dryrun(true);
    t.engine.set_rcfile("test.rc");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::DeleteInsert {
            field: "Subject".into(),
            value: "new".into(),
        },
        line: 5,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("Subject").as_deref(), Some("new"));
}

#[test]
fn dryrun_log_header_rename_insert() {
    let mut t = Test::with_msg("Subject: old\n\nbody");
    t.engine.set_dryrun(true);
    t.engine.set_rcfile("test.rc");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::RenameInsert {
            field: "Subject".into(),
            value: "new".into(),
        },
        line: 10,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("Subject").as_deref(), Some("new"));
    assert_eq!(t.msg.get_header("Old-Subject").as_deref(), Some("old"));
}

#[test]
fn dryrun_log_header_add_if_not() {
    let mut t = Test::with_msg("Subject: test\n\nbody");
    t.engine.set_dryrun(true);
    t.engine.set_rcfile("test.rc");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::AddIfNot {
            field: "X-New".into(),
            value: "added".into(),
        },
        line: 3,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("X-New").as_deref(), Some("added"));
}

#[test]
fn dryrun_log_header_add_always() {
    let mut t = Test::with_msg("X-Tag: first\n\nbody");
    t.engine.set_dryrun(true);
    t.engine.set_rcfile("test.rc");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::AddAlways {
            field: "X-Tag".into(),
            value: "second".into(),
        },
        line: 7,
    }];
    t.process(&items);
    let hdr = t.msg.header();
    let raw = str::from_utf8(&hdr).unwrap();
    assert!(raw.contains("X-Tag: second"));
}

#[test]
fn dryrun_log_header_expands_vars() {
    let mut t = Test::with_msg("Subject: old\n\nbody");
    t.engine.set_dryrun(true);
    t.engine.set_rcfile("test.rc");
    t.engine.set_var("WHO", "alice");
    let items = vec![Item::HeaderOp {
        op: HeaderOp::DeleteInsert {
            field: "X-By".into(),
            value: "$WHO".into(),
        },
        line: 1,
    }];
    t.process(&items);
    assert_eq!(t.msg.get_header("X-By").as_deref(), Some("alice"));
}

#[test]
fn shell_true_delivers() {
    let mut t = Test::new();
    let items = vec![shell_recipe("/bin/true", &t.maildir("inbox"))];
    delivered(t.process(&items));
}

#[test]
fn shell_false_skips() {
    let mut t = Test::new();
    let items = vec![shell_recipe("/bin/false", &t.maildir("inbox"))];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn shell_negated_true_skips() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: "/bin/true".into(),
                negate: true,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("inbox"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
}

#[test]
fn shell_negated_false_delivers() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: "/bin/false".into(),
                negate: true,
                weight: None,
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("inbox"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
}

#[test]
fn shell_sets_last_exit() {
    let mut t = Test::new();
    let items = vec![shell_recipe("exit 7", &t.maildir("inbox"))];
    assert_eq!(t.process(&items), Outcome::Default);
    assert_eq!(t.engine.ctx.last_exit, 7);
}

#[test]
fn shell_weighted_exit_zero() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: "/bin/true".into(),
                negate: false,
                weight: Some(Weight { w: 5.0, x: 2.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, 5);
}

#[test]
fn shell_weighted_exit_nonzero() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: "/bin/false".into(),
                negate: false,
                weight: Some(Weight { w: 5.0, x: 2.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, 2);
}

#[test]
fn shell_weighted_negated_loop() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: "exit 3".into(),
                negate: true,
                weight: Some(Weight { w: 2.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, 6);
}

#[test]
fn shell_weighted_negated_decay() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: "exit 3".into(),
                negate: true,
                weight: Some(Weight { w: 1.0, x: 0.5 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    delivered(t.process(&items));
    assert_eq!(t.engine.ctx.last_score, 1);
}

#[test]
fn shell_weighted_negated_exit_zero() {
    let mut t = Test::new();
    let items = vec![Item::Recipe {
        recipe: Recipe {
            flags: Flags::new(),
            lockfile: None,
            conds: vec![Condition::Shell {
                cmd: "/bin/true".into(),
                negate: true,
                weight: Some(Weight { w: 5.0, x: 1.0 }),
            }],
            action: Action::Folder(vec![PathBuf::from(t.maildir("w"))]),
        },
        line: 0,
    }];
    assert_eq!(t.process(&items), Outcome::Default);
    assert_eq!(t.engine.ctx.last_score, 0);
}

#[test]
fn shell_spawn_failure() {
    let mut t = Test::new();
    t.engine.set_var("SHELL", "/no/such/shell");
    let items = vec![shell_recipe("true", &t.maildir("inbox"))];
    assert!(t.try_process(&items).is_err());
}

#[test]
fn shell_timeout_fails() {
    let mut t = Test::new();
    t.engine.set_var("TIMEOUT", "1");
    let items = vec![shell_recipe("sleep 60", &t.maildir("inbox"))];
    assert_eq!(t.process(&items), Outcome::Default);
    assert_eq!(t.engine.ctx.last_exit, -1);
}

#[test]
fn trap_noop_when_unset() {
    let mut t = Test::new();
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), None);
    assert!(!t.engine.exit_was_set());
}

#[test]
fn trap_noop_when_empty() {
    let mut t = Test::new();
    t.engine.set_var("TRAP", "");
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), None);
    assert!(!t.engine.exit_was_set());
}

#[test]
fn trap_sets_exitcode_zero() {
    let mut t = Test::new();
    t.engine.set_var("TRAP", "true");
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), Some("0"));
    assert!(t.engine.exit_was_set());
}

// When EXITCODE is not set, TRAP exit code does NOT override (procmail
// forceret==-1).  Only EXITCODE= (empty) enables TRAP exit override.
#[test]
fn trap_nonzero_without_exitcode_set() {
    let mut t = Test::new();
    t.engine.set_var("TRAP", "exit 42");
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), Some("0"));
}

#[test]
fn trap_nonzero_with_empty_exitcode() {
    let mut t = Test::new();
    t.engine.set_var("EXITCODE", "");
    t.engine.set_var("TRAP", "exit 42");
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), Some("42"));
}

#[test]
fn trap_preserves_user_exitcode() {
    let mut t = Test::new();
    t.engine.set_var("EXITCODE", "99");
    t.engine.set_var("TRAP", "exit 1");
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), Some("99"));
}

#[test]
fn trap_expands_variables() {
    let mut t = Test::new();
    let marker = t.folder("marker");
    t.engine.set_var("MARKER", &marker.display().to_string());
    t.engine.set_var("TRAP", "touch $MARKER");
    t.engine.run_trap(&t.msg);
    assert!(marker.exists(), "TRAP did not expand $MARKER");
}

#[test]
fn trap_feeds_message() {
    let mut t = Test::with_msg("Subject: t\n\nBody");
    let out = t.folder("stdin");
    t.engine
        .set_var("TRAP", &format!("cat > {}", out.display()));
    t.engine.run_trap(&t.msg);
    let got = fs::read_to_string(&out).unwrap();
    assert!(got.contains("Body"), "stdin missing body: {got:?}");
}

#[test]
fn trap_spawn_failure() {
    let mut t = Test::new();
    t.engine.set_var("SHELL", "/no/such/shell");
    t.engine.set_var("TRAP", "true");
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), Some("0"));

    // user-set EXITCODE preserved on spawn failure
    let mut t = Test::new();
    t.engine.set_var("EXITCODE", "77");
    t.engine.set_var("SHELL", "/no/such/shell");
    t.engine.set_var("TRAP", "true");
    t.engine.run_trap(&t.msg);
    assert_eq!(t.engine.get_var("EXITCODE"), Some("77"));
}
