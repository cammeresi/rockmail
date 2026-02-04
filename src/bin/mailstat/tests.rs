use super::*;
use std::io::Cursor;

#[test]
fn digits_zero() {
    assert_eq!(digits(0), 1);
}

#[test]
fn digits_single() {
    for n in 1..=9 {
        assert_eq!(digits(n), 1, "digits({n})");
    }
}

#[test]
fn digits_boundaries() {
    assert_eq!(digits(10), 2);
    assert_eq!(digits(99), 2);
    assert_eq!(digits(100), 3);
    assert_eq!(digits(999), 3);
    assert_eq!(digits(1000), 4);
}

#[test]
fn normalize_plain() {
    assert_eq!(normalize_folder("/var/mail/inbox"), "/var/mail/inbox");
}

#[test]
fn normalize_trailing_slash() {
    assert_eq!(normalize_folder("/var/mail/inbox/"), "/var/mail/inbox");
}

#[test]
fn normalize_maildir() {
    assert_eq!(
        normalize_folder("/home/user/Maildir/new/1234567890.12345.host"),
        "/home/user/Maildir"
    );
}

#[test]
fn parse_empty() {
    let input = Cursor::new("");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert!(totals.is_empty());
}

#[test]
fn parse_folder_lines() {
    let input = Cursor::new(
        "From foo@bar.com
 Subject: test
  Folder: /var/mail/inbox 1234
",
    );
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 1);
    let s = totals.get("/var/mail/inbox").unwrap();
    assert_eq!(s.msgs, 1);
    assert_eq!(s.bytes, 1234);
}

#[test]
fn parse_multiple_folders() {
    let input = Cursor::new(
        "  Folder: /mail/a 100\n\
         \n\
         Folder: /mail/b 200\n\
         \n\
         Folder: /mail/a 150\n",
    );
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    let a = totals.get("/mail/a").unwrap();
    assert_eq!(a.msgs, 1);
    assert_eq!(a.bytes, 100);
}

#[test]
fn parse_errors_merged() {
    let input = Cursor::new("some error line\nanother error\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, true, false).unwrap();
    assert_eq!(totals.len(), 1);
    let s = totals.get("## diagnostic messages ##").unwrap();
    assert_eq!(s.msgs, 2);
}

#[test]
fn parse_errors_separate() {
    let input = Cursor::new("error one\nerror two\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 2);
    assert!(totals.contains_key("## error one"));
    assert!(totals.contains_key("## error two"));
}

#[test]
fn parse_writes_to_writer() {
    let input = Cursor::new("  Folder: /mail/a 100\n");
    let mut out = Vec::new();
    parse_log(input, Some(&mut out), false, false).unwrap();
    assert_eq!(out, b"  Folder: /mail/a 100\n");
}

#[test]
fn parse_old_ignores_errors() {
    let input = Cursor::new("error line\n  Folder: /mail/a 100\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, true).unwrap();
    assert_eq!(totals.len(), 1);
    assert!(totals.contains_key("/mail/a"));
    assert!(!totals.contains_key("## error line"));
}

#[test]
fn parse_accumulates_bytes() {
    let input = Cursor::new("  Folder: /mail/a 100\n  Folder: /mail/a 150\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    let a = totals.get("/mail/a").unwrap();
    assert_eq!(a.msgs, 2);
    assert_eq!(a.bytes, 250);
}

#[test]
fn parse_malformed_folder_missing_size() {
    let input = Cursor::new("  Folder: /mail/a\n  Folder: /mail/b 200\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 2);
    assert!(totals.contains_key("/mail/b"));
    assert!(totals.contains_key("## Folder: /mail/a"));
}

#[test]
fn parse_malformed_folder_bad_size() {
    let input = Cursor::new("  Folder: /mail/a xyz\n  Folder: /mail/b 200\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 2);
    assert!(totals.contains_key("/mail/b"));
    assert!(totals.contains_key("## Folder: /mail/a xyz"));
}

#[test]
fn input_path_normal() {
    assert_eq!(
        input_path(Path::new("/var/log/procmail"), false),
        PathBuf::from("/var/log/procmail")
    );
}

#[test]
fn input_path_old() {
    assert_eq!(
        input_path(Path::new("/var/log/procmail"), true),
        PathBuf::from("/var/log/procmail.old")
    );
}

#[test]
fn stats_add() {
    let mut s = Stats::default();
    s.add(100);
    s.add(50);
    assert_eq!(s.msgs, 2);
    assert_eq!(s.bytes, 150);
}

#[test]
fn filter_ignores() {
    let mut totals = BTreeMap::new();
    totals.insert(
        "keep".into(),
        Stats {
            msgs: 1,
            bytes: 100,
        },
    );
    totals.insert(
        "drop".into(),
        Stats {
            msgs: 2,
            bytes: 200,
        },
    );
    let ignores = HashSet::from(["drop".into()]);
    let filtered = filter_totals(totals, &ignores, false);
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains_key("keep"));
}

#[test]
fn filter_errors() {
    let mut totals = BTreeMap::new();
    totals.insert("## error".into(), Stats { msgs: 1, bytes: 0 });
    totals.insert(
        "folder".into(),
        Stats {
            msgs: 2,
            bytes: 200,
        },
    );
    let filtered = filter_totals(totals, &HashSet::new(), true);
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains_key("folder"));
}
