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

#[test]
fn filter_keeps_errors_when_not_quiet() {
    let mut totals = BTreeMap::new();
    totals.insert("## error".into(), Stats { msgs: 1, bytes: 0 });
    totals.insert(
        "folder".into(),
        Stats {
            msgs: 2,
            bytes: 200,
        },
    );
    let filtered = filter_totals(totals, &HashSet::new(), false);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn filter_empty() {
    let totals = BTreeMap::new();
    let filtered = filter_totals(totals, &HashSet::new(), false);
    assert!(filtered.is_empty());
}

#[test]
fn widths_basic() {
    let mut totals = BTreeMap::new();
    totals.insert(
        "/mail/inbox".into(),
        Stats {
            msgs: 3,
            bytes: 900,
        },
    );
    let w = Widths::compute(&totals, 3, 900);
    // avg = 900/3 = 300 -> 3 digits, but min is "Average".len() = 7
    assert_eq!(w.avg, 7);
    // bytes = 900 -> 3 digits, but min is "Total".len() = 5
    assert_eq!(w.bytes, 5);
    // msgs = 3 -> 1 digit, but min is "Number".len() = 6
    assert_eq!(w.msgs, 6);
    // folder = "/mail/inbox".len() = 11 > "Folder".len() = 6
    assert_eq!(w.folder, 11);
}

#[test]
fn widths_empty() {
    let totals = BTreeMap::new();
    let w = Widths::compute(&totals, 0, 0);
    assert_eq!(w.avg, 7); // "Average".len()
    assert_eq!(w.bytes, 5); // "Total".len()
    assert_eq!(w.msgs, 6); // "Number".len()
    assert_eq!(w.folder, 6); // "Folder".len()
}

#[test]
fn widths_large_values() {
    let mut totals = BTreeMap::new();
    totals.insert(
        "x".into(),
        Stats {
            msgs: 1,
            bytes: 1_000_000,
        },
    );
    let w = Widths::compute(&totals, 1, 1_000_000);
    // bytes = 1000000 -> 7 digits > "Total".len() = 5
    assert_eq!(w.bytes, 7);
    // avg = 1000000 -> 7 digits > "Average".len() = 7
    assert_eq!(w.avg, 7);
}

#[test]
fn normalize_maildir_trailing_slash() {
    // Trailing slash prevents the /new/ regex from matching,
    // so only the slash is stripped.
    assert_eq!(
        normalize_folder("/home/user/Maildir/new/msg123/"),
        "/home/user/Maildir/new/msg123"
    );
}

#[test]
fn normalize_bare_name() {
    assert_eq!(normalize_folder("inbox"), "inbox");
}

#[test]
fn parse_skips_from_lines() {
    let input = Cursor::new("From someone@host\n  Folder: /mail/a 50\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 1);
    assert!(!totals.contains_key("## From someone@host"));
}

#[test]
fn parse_skips_subject_lines() {
    let input = Cursor::new(" Subject: hello\n  Folder: /mail/a 50\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 1);
    assert!(!totals.contains_key("## Subject: hello"));
}

#[test]
fn parse_skips_empty_lines() {
    let input = Cursor::new("\n\n  Folder: /mail/a 50\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 1);
}

#[test]
fn parse_maildir_normalized() {
    let input = Cursor::new("  Folder: /home/x/Maildir/new/12345.host 300\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert!(totals.contains_key("/home/x/Maildir"));
    assert_eq!(totals.len(), 1);
}

#[test]
fn parse_writer_multiple_lines() {
    let input = Cursor::new("line one\nline two\n");
    let mut out = Vec::new();
    parse_log(input, Some(&mut out), false, false).unwrap();
    assert_eq!(out, b"line one\nline two\n");
}

#[test]
fn parse_rc_empty() {
    let rc = parse_rc(Cursor::new(""));
    assert!(rc.ignores.is_empty());
    assert!(rc.date_fmt.is_none());
}

#[test]
fn parse_rc_ignore() {
    let rc = parse_rc(Cursor::new("ignore /dev/null\nignore /mail/spam\n"));
    assert_eq!(rc.ignores.len(), 2);
    assert!(rc.ignores.contains("/dev/null"));
    assert!(rc.ignores.contains("/mail/spam"));
}

#[test]
fn parse_rc_date_format() {
    let rc = parse_rc(Cursor::new("date_format %Y-%m-%d\n"));
    assert_eq!(rc.date_fmt.as_deref(), Some("%Y-%m-%d"));
}

#[test]
fn parse_rc_date_format_last_wins() {
    let rc = parse_rc(Cursor::new("date_format %Y\ndate_format %m-%d\n"));
    assert_eq!(rc.date_fmt.as_deref(), Some("%m-%d"));
}

#[test]
fn parse_rc_blank_lines_skipped() {
    let rc = parse_rc(Cursor::new("\n  \n\nignore foo\n\n"));
    assert_eq!(rc.ignores.len(), 1);
    assert!(rc.ignores.contains("foo"));
}

#[test]
fn parse_rc_mixed() {
    let rc = parse_rc(Cursor::new(
        "ignore /dev/null\ndate_format %H:%M\nignore spam\n",
    ));
    assert_eq!(rc.ignores.len(), 2);
    assert_eq!(rc.date_fmt.as_deref(), Some("%H:%M"));
}

#[test]
fn digits_large() {
    assert_eq!(digits(1_000_000), 7);
    assert_eq!(digits(999_999), 6);
    assert_eq!(digits(10_000_000), 8);
}

#[test]
fn stats_default() {
    let s = Stats::default();
    assert_eq!(s.msgs, 0);
    assert_eq!(s.bytes, 0);
}

#[test]
fn parse_subject_case_insensitive() {
    let input = Cursor::new(" SUBJECT: HELLO\n  Folder: /mail/a 50\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert_eq!(totals.len(), 1);
    assert!(!totals.contains_key("## SUBJECT: HELLO"));
}

#[test]
fn parse_error_trimmed() {
    let input = Cursor::new("  some error  \n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert!(totals.contains_key("## some error"));
}

#[test]
fn parse_merged_errors_zero_bytes() {
    let input = Cursor::new("err1\nerr2\nerr3\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, true, false).unwrap();
    let s = totals.get("## diagnostic messages ##").unwrap();
    assert_eq!(s.msgs, 3);
    assert_eq!(s.bytes, 0);
}

#[test]
fn parse_mixed_folders_and_errors() {
    let input = Cursor::new(
        "  Folder: /mail/a 100\n\
         bad line\n\
         From user@host\n\
         \n\
          Subject: hi\n\
         another error\n\
         Folder: /mail/b 200\n",
    );
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert!(totals.contains_key("/mail/a"));
    assert!(totals.contains_key("## bad line"));
    assert!(totals.contains_key("## another error"));
    // "Folder:" without leading two-space indent is an error
    assert!(totals.contains_key("## Folder: /mail/b 200"));
    // From, Subject, blank are skipped
    assert!(!totals.contains_key("## From user@host"));
    assert_eq!(totals.get("/mail/a").unwrap().bytes, 100);
}

#[test]
fn widths_zero_msg_entry() {
    // Entry with msgs=0 is filtered out of avg computation
    let mut totals = BTreeMap::new();
    totals.insert("err".into(), Stats { msgs: 0, bytes: 0 });
    totals.insert(
        "/mail/a".into(),
        Stats {
            msgs: 2,
            bytes: 200,
        },
    );
    let w = Widths::compute(&totals, 2, 200);
    // avg from /mail/a only: 200/2=100 -> 3 digits, min 7
    assert_eq!(w.avg, 7);
}

#[test]
fn widths_all_zero_msgs() {
    let mut totals = BTreeMap::new();
    totals.insert("err".into(), Stats { msgs: 0, bytes: 0 });
    let w = Widths::compute(&totals, 0, 0);
    // No entry passes the filter, unwrap_or(1), min 7
    assert_eq!(w.avg, 7);
}

#[test]
fn filter_ignore_and_quiet_combined() {
    let mut totals = BTreeMap::new();
    totals.insert("## err".into(), Stats { msgs: 1, bytes: 0 });
    totals.insert(
        "ignored".into(),
        Stats {
            msgs: 1,
            bytes: 100,
        },
    );
    totals.insert(
        "kept".into(),
        Stats {
            msgs: 1,
            bytes: 200,
        },
    );
    let ignores = HashSet::from(["ignored".into()]);
    let filtered = filter_totals(totals, &ignores, true);
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains_key("kept"));
}

#[test]
fn normalize_multiple_trailing_slashes() {
    assert_eq!(normalize_folder("/var/mail/inbox///"), "/var/mail/inbox");
}

#[test]
fn normalize_just_slashes() {
    assert_eq!(normalize_folder("///"), "");
}

#[test]
fn open_old_file_create() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("log");
    let w = open_old_file(&base, false);
    assert!(w.is_ok());
    assert!(base.with_added_extension("old").exists());
}

#[test]
fn open_old_file_preserve() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("log.old");
    fs::write(&old, "existing\n").unwrap();
    let base = dir.path().join("log");
    let mut w = open_old_file(&base, true).unwrap();
    writeln!(w, "appended").unwrap();
    w.flush().unwrap();
    let content = fs::read_to_string(&old).unwrap();
    assert_eq!(content, "existing\nappended\n");
}

#[test]
fn open_old_file_bad_path() {
    let result = open_old_file(Path::new("/no/such/dir/log"), false);
    assert_eq!(result.unwrap_err(), EX_CANTCREAT);
}

#[test]
fn truncate_and_preserve_mtime_works() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(&path, "hello").unwrap();
    let target = FileTime::from_unix_time(1_000_000, 0);
    truncate_and_preserve_mtime(&path, target);
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
    let actual =
        FileTime::from_last_modification_time(&fs::metadata(&path).unwrap());
    assert_eq!(actual, target);
}

#[test]
fn print_no_mail_silent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    fs::write(&path, "").unwrap();
    let meta = fs::metadata(&path).unwrap();
    let mut buf = Vec::new();
    assert_eq!(print_no_mail(&mut buf, &meta, &None, true), EX_OK);
    assert!(buf.is_empty());
}

#[test]
fn print_no_mail_with_date_fmt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    fs::write(&path, "").unwrap();
    let meta = fs::metadata(&path).unwrap();
    let fmt = Some("%Y-%m-%d".to_string());
    let mut buf = Vec::new();
    assert_eq!(print_no_mail(&mut buf, &meta, &fmt, false), EX_OK);
    let s = output(&buf);
    assert!(s.starts_with("No mail arrived since "));
}

#[test]
fn print_no_mail_default_fmt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    fs::write(&path, "").unwrap();
    let meta = fs::metadata(&path).unwrap();
    let mut buf = Vec::new();
    assert_eq!(print_no_mail(&mut buf, &meta, &None, false), EX_OK);
    let s = output(&buf);
    assert!(s.starts_with("No mail arrived since "));
}

#[test]
fn process_log_keep() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "  Folder: /mail/a 100\n").unwrap();
    let args = Args::parse_from(["mailstat", "-k", log.to_str().unwrap()]);
    let totals = process_log(&args, &log, true, false).unwrap();
    assert!(totals.contains_key("/mail/a"));
    // Original file untouched in keep mode
    assert_eq!(fs::read_to_string(&log).unwrap(), "  Folder: /mail/a 100\n");
}

#[test]
fn process_log_copies_to_old() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "  Folder: /mail/b 200\n").unwrap();
    let args = Args::parse_from(["mailstat", log.to_str().unwrap()]);
    let totals = process_log(&args, &log, false, false).unwrap();
    assert!(totals.contains_key("/mail/b"));
    let old = fs::read_to_string(log.with_added_extension("old")).unwrap();
    assert_eq!(old, "  Folder: /mail/b 200\n");
}

#[test]
fn process_log_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    let args = Args::parse_from(["mailstat", "-k", log.to_str().unwrap()]);
    assert_eq!(
        process_log(&args, &log, true, false).unwrap_err(),
        EX_NOINPUT
    );
}

#[test]
fn parse_folder_size_zero() {
    let input = Cursor::new("  Folder: /mail/a 0\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    let s = totals.get("/mail/a").unwrap();
    assert_eq!(s.msgs, 1);
    assert_eq!(s.bytes, 0);
}

#[test]
fn parse_only_skippable_lines() {
    let input = Cursor::new("From a@b\n\n Subject: x\nFrom c@d\n");
    let totals = parse_log(input, None::<&mut Vec<u8>>, false, false).unwrap();
    assert!(totals.is_empty());
}

fn sample_totals() -> BTreeMap<String, Stats> {
    let mut t = BTreeMap::new();
    t.insert(
        "/mail/a".into(),
        Stats {
            msgs: 2,
            bytes: 300,
        },
    );
    t.insert(
        "/mail/b".into(),
        Stats {
            msgs: 1,
            bytes: 500,
        },
    );
    t
}

fn output(buf: &[u8]) -> &str {
    std::str::from_utf8(buf).unwrap()
}

#[test]
fn write_stats_short() {
    let t = sample_totals();
    let mut buf = Vec::new();
    write_stats(&mut buf, &t, 3, 800, false, false);
    let s = output(&buf);
    assert!(s.contains("Total"));
    assert!(s.contains("Number"));
    assert!(!s.contains("Average"));
    assert!(s.contains("/mail/a"));
    assert!(s.contains("/mail/b"));
    assert!(s.contains("300"));
    assert!(s.contains("500"));
    assert!(s.contains("800"));
}

#[test]
fn write_stats_long() {
    let t = sample_totals();
    let mut buf = Vec::new();
    write_stats(&mut buf, &t, 3, 800, true, false);
    let s = output(&buf);
    assert!(s.contains("Average"));
    assert!(s.contains("150")); // avg for /mail/a: 300/2
    assert!(s.contains("500")); // avg for /mail/b: 500/1
}

#[test]
fn write_stats_terse() {
    let t = sample_totals();
    let mut buf = Vec::new();
    write_stats(&mut buf, &t, 3, 800, false, true);
    let s = output(&buf);
    // No header/footer
    assert!(!s.contains("Total"));
    assert!(!s.contains("Number"));
    // Rows present without leading "  " prefix
    assert!(s.contains("/mail/a"));
    assert!(s.contains("/mail/b"));
    // No separator dashes
    assert!(!s.contains("---"));
}

#[test]
fn format_row_zero_msgs() {
    let w = Widths {
        bytes: 5,
        avg: 7,
        msgs: 6,
        folder: 6,
    };
    let s = Stats { msgs: 0, bytes: 0 };
    let mut buf = Vec::new();
    Format::Long.row(&mut buf, &w, "  ", &s, "empty");
    let out = output(&buf);
    // avg should be 0 when msgs is 0
    assert!(out.contains("0"));
    assert!(out.contains("empty"));
}

#[test]
fn format_footer_zero_msgs() {
    let w = Widths {
        bytes: 5,
        avg: 7,
        msgs: 6,
        folder: 6,
    };
    let mut buf = Vec::new();
    Format::Long.footer(&mut buf, &w, 0, 0);
    let out = output(&buf);
    assert!(out.contains("0"));
}

#[test]
fn format_header_short() {
    let w = Widths {
        bytes: 5,
        avg: 7,
        msgs: 6,
        folder: 6,
    };
    let mut buf = Vec::new();
    Format::Short.header(&mut buf, &w);
    let out = output(&buf);
    assert!(out.contains("Total"));
    assert!(out.contains("Number"));
    assert!(out.contains("Folder"));
    assert!(!out.contains("Average"));
}

#[test]
fn format_header_long() {
    let w = Widths {
        bytes: 5,
        avg: 7,
        msgs: 6,
        folder: 6,
    };
    let mut buf = Vec::new();
    Format::Long.header(&mut buf, &w);
    let out = output(&buf);
    assert!(out.contains("Average"));
}

#[test]
fn format_sep_short() {
    let w = Widths {
        bytes: 5,
        avg: 7,
        msgs: 6,
        folder: 6,
    };
    let mut buf = Vec::new();
    Format::Short.sep(&mut buf, &w);
    let out = output(&buf);
    assert!(out.contains("-----"));
    assert!(out.contains("------"));
}

#[test]
fn format_sep_long() {
    let w = Widths {
        bytes: 5,
        avg: 7,
        msgs: 6,
        folder: 6,
    };
    let mut buf = Vec::new();
    Format::Long.sep(&mut buf, &w);
    let out = output(&buf);
    assert!(out.contains("-------")); // avg width = 7
}

#[test]
fn run_missing_logfile() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("noexist");
    let args = Args::parse_from(["mailstat", "-k", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_NOINPUT);
    assert!(buf.is_empty());
}

#[test]
fn run_empty_logfile_silent() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "").unwrap();
    let args =
        Args::parse_from(["mailstat", "-k", "-s", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    assert!(buf.is_empty());
}

#[test]
fn run_empty_logfile_not_silent() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "").unwrap();
    let args = Args::parse_from(["mailstat", "-k", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    assert!(output(&buf).starts_with("No mail arrived since "));
}

#[test]
fn run_keep_mode() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "  Folder: /mail/a 100\n").unwrap();
    let args = Args::parse_from(["mailstat", "-k", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    let s = output(&buf);
    assert!(s.contains("/mail/a"));
    assert!(s.contains("100"));
    // File should be untouched
    assert_eq!(fs::read_to_string(&log).unwrap(), "  Folder: /mail/a 100\n");
}

#[test]
fn run_truncates_without_keep() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "  Folder: /mail/a 100\n").unwrap();
    let args = Args::parse_from(["mailstat", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, false), EX_OK);
    assert!(output(&buf).contains("/mail/a"));
    // Log should be truncated
    assert_eq!(fs::read_to_string(&log).unwrap(), "");
    // Content should be in .old
    let old = fs::read_to_string(log.with_added_extension("old")).unwrap();
    assert_eq!(old, "  Folder: /mail/a 100\n");
}

#[test]
fn run_old_mode() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    let old = log.with_added_extension("old");
    fs::write(&old, "  Folder: /mail/b 200\n").unwrap();
    let args = Args::parse_from(["mailstat", "-o", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    assert!(output(&buf).contains("/mail/b"));
    assert_eq!(fs::read_to_string(&old).unwrap(), "  Folder: /mail/b 200\n");
}

#[test]
fn run_quiet_filters_errors() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "some error\n").unwrap();
    let args =
        Args::parse_from(["mailstat", "-k", "-i", "-s", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    assert!(buf.is_empty());
}

#[test]
fn run_all_ignored_falls_to_no_mail() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "error one\nerror two\n").unwrap();
    let args =
        Args::parse_from(["mailstat", "-k", "-i", "-s", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    assert!(buf.is_empty());
}

#[test]
fn run_long_format() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "  Folder: /mail/a 300\n  Folder: /mail/a 300\n").unwrap();
    let args =
        Args::parse_from(["mailstat", "-k", "-l", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    let s = output(&buf);
    assert!(s.contains("Average"));
    assert!(s.contains("300")); // avg = 600/2
}

#[test]
fn run_terse_format() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    fs::write(&log, "  Folder: /mail/a 100\n").unwrap();
    let args =
        Args::parse_from(["mailstat", "-k", "-t", log.to_str().unwrap()]);
    let mut buf = Vec::new();
    assert_eq!(run(&mut buf, &args, true), EX_OK);
    let s = output(&buf);
    assert!(s.contains("/mail/a"));
    assert!(!s.contains("Total"));
    assert!(!s.contains("---"));
}
