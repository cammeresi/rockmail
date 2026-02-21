use tempfile::TempDir;

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
    assert_eq!(t, FolderType::Maildir);
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
    assert_eq!(t, FolderType::Maildir);
    assert_eq!(p, "/home/user/backup");
}

#[test]
fn parse_existing_dir() {
    let d = TempDir::new().unwrap();
    let p = d.path().to_str().unwrap();
    let (t, _) = FolderType::parse(p);
    assert_eq!(t, FolderType::Dir);
}

#[test]
fn update_perms_sets_execute_other() {
    let d = TempDir::new().unwrap();
    let p = d.path().join("file");
    fs::write(&p, b"").unwrap();
    fs::set_permissions(&p, Permissions::from_mode(0o644)).unwrap();

    update_perms(&p, 0);

    let mode = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o645);
}

#[test]
fn update_perms_skipped_when_umask_blocks() {
    let d = TempDir::new().unwrap();
    let p = d.path().join("file");
    fs::write(&p, b"").unwrap();
    fs::set_permissions(&p, Permissions::from_mode(0o644)).unwrap();

    update_perms(&p, 0o001);

    let mode = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644);
}

#[test]
fn update_perms_skipped_for_dev() {
    update_perms(Path::new("/dev/null"), 0);
}

#[test]
fn link_secondary_maildir() {
    let d = TempDir::new().unwrap();
    let src = d.path().join("src");
    fs::write(&src, b"msg").unwrap();

    let dst = d.path().join("maildir");
    let mut namer = Namer::new();
    let r = link_secondary(&src, &dst, FolderType::Maildir, &mut namer, "msg.");
    assert!(r.is_ok());
    assert!(dst.join("new").is_dir());
}

#[test]
fn link_secondary_mh() {
    let d = TempDir::new().unwrap();
    let src = d.path().join("src");
    fs::write(&src, b"msg").unwrap();

    let dst = d.path().join("mh");
    let mut namer = Namer::new();
    let r = link_secondary(&src, &dst, FolderType::Mh, &mut namer, "");
    assert!(r.is_ok());
    assert!(dst.join("1").exists());
}

#[test]
fn link_secondary_dir() {
    let d = TempDir::new().unwrap();
    let src = d.path().join("src");
    fs::write(&src, b"msg").unwrap();

    let dst = d.path().join("dir");
    let mut namer = Namer::new();
    let r = link_secondary(&src, &dst, FolderType::Dir, &mut namer, "msg.");
    let path = r.unwrap();
    assert!(path.starts_with(&dst.display().to_string()));
}

#[test]
fn link_secondary_bad_source() {
    let d = TempDir::new().unwrap();
    let src = d.path().join("nonexistent");
    let dst = d.path().join("maildir");
    let mut namer = Namer::new();

    let r = link_secondary(&src, &dst, FolderType::Maildir, &mut namer, "");
    let Err(DeliveryError::Io { op, .. }) = r else {
        panic!("expected Io error, got {r:?}");
    };
    assert_eq!(op, "link");
}

#[test]
fn link_secondary_readonly_parent() {
    let d = TempDir::new().unwrap();
    let src = d.path().join("src");
    fs::write(&src, b"msg").unwrap();

    let parent = d.path().join("readonly");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, Permissions::from_mode(0o444)).unwrap();

    let dst = parent.join("mh");
    let mut namer = Namer::new();
    let r = link_secondary(&src, &dst, FolderType::Mh, &mut namer, "");
    let Err(DeliveryError::Io { op, .. }) = r else {
        panic!("expected Io error, got {r:?}");
    };
    assert_eq!(op, "create");

    fs::set_permissions(&parent, Permissions::from_mode(0o755)).unwrap();
}
