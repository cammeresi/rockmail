//! Property-based gold tests using proptest.
//!
//! Generates random rcfiles and messages from a limited dictionary,
//! then verifies rockmail and procmail produce identical results.
//!
//! Run with:
//!     cargo test --features gold --test rockmail_proptest

#![cfg(feature = "gold")]

use proptest::prelude::*;

use rockmail::delivery::FolderType;

#[allow(unused)]
mod common;
mod gold;

use common::RcBuilder;
use gold::{ADDRS, GoldTest, LISTS, SUBJECTS};

const BODIES: &[&str] = &[
    "Body\n",
    "Hello world\n",
    "test test test\n",
    "URGENT please respond\n",
    "short\n",
];

const EXTRAS: &[&str] = &["", "", "", "X-Spam-Flag: YES", "X-Priority: 1"];

#[derive(Debug, Clone)]
enum Rule {
    Spam,
    Lists,
    Priority,
    From(usize),
    Subject(usize),
    SizeGt(usize),
    SizeLt(usize),
}

#[derive(Debug, Clone)]
struct RuleSpec {
    kind: Rule,
    copy: bool,
    negate: bool,
}

#[derive(Debug, Clone)]
struct RcSpec {
    folder: usize,
    rules: Vec<RuleSpec>,
}

#[derive(Debug, Clone)]
struct MsgSpec {
    from: usize,
    to: usize,
    subj: usize,
    extra: usize,
    body: usize,
}

const FOLDER_SUFFIXES: [&str; 3] = ["", "/", "/."];

fn folder_type(idx: usize) -> FolderType {
    match idx {
        0 => FolderType::File,
        1 => FolderType::Maildir,
        _ => FolderType::Mh,
    }
}

fn folder_name(rule: &Rule) -> &'static str {
    match rule {
        Rule::Spam => "spam",
        Rule::Lists => "lists",
        Rule::Priority => "urgent",
        Rule::From(i) => ADDRS[*i].split('@').next().unwrap(),
        Rule::Subject(_) => "subj",
        Rule::SizeGt(_) => "big",
        Rule::SizeLt(_) => "small",
    }
}

fn build_rc(spec: &RcSpec) -> String {
    let kind = folder_type(spec.folder);
    let mut b = RcBuilder::new(kind);
    for r in &spec.rules {
        if r.copy {
            b.copy();
        }
        if r.negate {
            b.negate();
        }
        match &r.kind {
            Rule::Spam => {
                b.spam();
            }
            Rule::Lists => {
                b.lists();
            }
            Rule::Priority => {
                b.priority();
            }
            Rule::From(i) => {
                b.from(ADDRS[*i]);
            }
            Rule::Subject(i) => {
                b.subject(SUBJECTS[*i]);
            }
            Rule::SizeGt(n) => {
                b.size_gt(*n);
            }
            Rule::SizeLt(n) => {
                b.size_lt(*n);
            }
        }
        b.folder(folder_name(&r.kind));
    }
    b.build()
}

fn build_msg(spec: &MsgSpec) -> Vec<u8> {
    let from = ADDRS[spec.from];
    let to = if spec.to < ADDRS.len() {
        ADDRS[spec.to]
    } else {
        LISTS[spec.to - ADDRS.len()]
    };
    let subj = SUBJECTS[spec.subj];
    let extra = EXTRAS[spec.extra];
    let body = BODIES[spec.body];
    let hdrs = if extra.is_empty() {
        format!("From: {from}\nTo: {to}\nSubject: {subj}")
    } else {
        format!("From: {from}\nTo: {to}\nSubject: {subj}\n{extra}")
    };
    format!("{hdrs}\n\n{body}").into_bytes()
}

fn arb_rule() -> impl Strategy<Value = RuleSpec> {
    let kind = prop_oneof![
        Just(Rule::Spam),
        Just(Rule::Lists),
        Just(Rule::Priority),
        (0..ADDRS.len()).prop_map(Rule::From),
        (0..SUBJECTS.len()).prop_map(Rule::Subject),
        (100..400usize).prop_map(Rule::SizeGt),
        (100..400usize).prop_map(Rule::SizeLt),
    ];
    (
        kind,
        proptest::bool::weighted(0.15),
        proptest::bool::weighted(0.2),
    )
        .prop_map(|(kind, copy, negate)| RuleSpec { kind, copy, negate })
}

fn arb_rc() -> impl Strategy<Value = RcSpec> {
    let folder = 0..FOLDER_SUFFIXES.len();
    let rules = proptest::collection::vec(arb_rule(), 1..=5);
    (folder, rules).prop_map(|(folder, rules)| RcSpec { folder, rules })
}

fn arb_msg() -> impl Strategy<Value = MsgSpec> {
    let to_max = ADDRS.len() + LISTS.len();
    (
        0..ADDRS.len(),
        0..to_max,
        0..SUBJECTS.len(),
        0..EXTRAS.len(),
        0..BODIES.len(),
    )
        .prop_map(|(from, to, subj, extra, body)| MsgSpec {
            from,
            to,
            subj,
            extra,
            body,
        })
}

fn arb_input() -> impl Strategy<Value = Vec<MsgSpec>> {
    proptest::collection::vec(arb_msg(), 3..=10)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]
    #[test]
    fn gold_proptest(rc in arb_rc(), msgs in arb_input()) {
        let rc = build_rc(&rc);
        let msgs: Vec<Vec<u8>> = msgs.iter().map(build_msg).collect();
        let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
        GoldTest::new(&rc, &refs).run();
    }
}
