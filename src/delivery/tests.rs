use super::*;
use crate::mail::Message;

pub fn msg(s: &str) -> Message {
    Message::parse(s.as_bytes())
}

#[test]
fn parse_mbox() {
    let (t, p) = FolderType::parse("/var/mail/user");
    assert_eq!(t, FolderType::File);
    assert_eq!(p, "/var/mail/user");
}

#[test]
fn parse_maildir() {
    let (t, p) = FolderType::parse("/home/user/Maildir/");
    assert_eq!(t, FolderType::Maildir);
    assert_eq!(p, "/home/user/Maildir");
}

#[test]
fn parse_maildir_trailing_slashes() {
    let (t, p) = FolderType::parse("/home/user/Maildir///");
    assert_eq!(t, FolderType::Dir);
    assert_eq!(p, "/home/user/Maildir");
}

#[test]
fn parse_mh() {
    let (t, p) = FolderType::parse("/home/user/Mail/inbox/.");
    assert_eq!(t, FolderType::Mh);
    assert_eq!(p, "/home/user/Mail/inbox");
}

#[test]
fn parse_mh_trailing_slashes() {
    let (t, p) = FolderType::parse("/home/user/Mail/inbox//.");
    assert_eq!(t, FolderType::Mh);
    assert_eq!(p, "/home/user/Mail/inbox");
}

#[test]
fn parse_dir() {
    let (t, p) = FolderType::parse("/home/user/backup//");
    assert_eq!(t, FolderType::Dir);
    assert_eq!(p, "/home/user/backup");
}
