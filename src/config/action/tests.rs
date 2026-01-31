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
