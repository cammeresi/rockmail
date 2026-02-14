use super::*;

#[test]
fn folder() {
    match Action::parse_line("/var/mail/spam") {
        Action::Folder(p) => {
            assert_eq!(p.to_str().unwrap(), "/var/mail/spam")
        }
        _ => panic!("expected folder"),
    }
}

#[test]
fn pipe() {
    match Action::parse_line("| /usr/bin/spamassassin") {
        Action::Pipe { cmd, capture } => {
            assert_eq!(cmd, "/usr/bin/spamassassin");
            assert!(capture.is_none());
        }
        _ => panic!("expected pipe"),
    }
}

#[test]
fn pipe_capture() {
    match Action::parse_line("RESULT=| /usr/bin/filter") {
        Action::Pipe { cmd, capture } => {
            assert_eq!(cmd, "/usr/bin/filter");
            assert_eq!(capture.unwrap(), "RESULT");
        }
        _ => panic!("expected pipe with capture"),
    }
}

#[test]
fn forward() {
    match Action::parse_line("! admin@example.com backup@example.com") {
        Action::Forward(addrs) => {
            assert_eq!(addrs.len(), 2);
            assert_eq!(addrs[0], "admin@example.com");
        }
        _ => panic!("expected forward"),
    }
}

#[test]
fn maildir() {
    match Action::parse_line("Maildir/") {
        Action::Folder(p) => assert_eq!(p.to_str().unwrap(), "Maildir/"),
        _ => panic!("expected folder"),
    }
}

#[test]
fn empty_forward_becomes_folder() {
    match Action::parse_line("!") {
        Action::Folder(p) => assert_eq!(p.to_str().unwrap(), "!"),
        _ => panic!("expected folder"),
    }
}

#[test]
fn pipe_capture_empty_name() {
    // =| cmd — empty var name, not a valid capture
    match Action::parse_line("=| /bin/cmd") {
        Action::Folder(_) => {}
        _ => panic!("expected folder for empty var name"),
    }
}

#[test]
fn pipe_capture_underscore() {
    match Action::parse_line("_=| /bin/cmd") {
        Action::Pipe { capture, .. } => assert_eq!(capture.unwrap(), "_"),
        _ => panic!("expected pipe capture"),
    }
}

#[test]
fn pipe_capture_space_before_eq() {
    // VAR =| cmd — space before = makes it not a var name
    match Action::parse_line("VAR =| /bin/cmd") {
        Action::Folder(_) => {}
        _ => panic!("expected folder when space before ="),
    }
}

#[test]
fn pipe_capture_space_after_eq() {
    // VAR= | cmd — space between = and | is OK
    match Action::parse_line("VAR= | /bin/cmd") {
        Action::Pipe { cmd, capture } => {
            assert_eq!(capture.unwrap(), "VAR");
            assert_eq!(cmd, "/bin/cmd");
        }
        _ => panic!("expected pipe capture"),
    }
}

#[test]
fn pipe_capture_invalid_name() {
    match Action::parse_line("123=| /bin/cmd") {
        Action::Folder(_) => {}
        _ => panic!("expected folder for invalid var name"),
    }
}
