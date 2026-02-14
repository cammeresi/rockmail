use super::*;

#[test]
fn field_new() {
    let f = Field::new(b"Subject: Hello\n".to_vec()).unwrap();
    assert_eq!(f.name(), b"Subject");
    assert_eq!(f.value(), b" Hello");
}

#[test]
fn field_from_parts() {
    let f = Field::from_parts(b"Subject:", b"Test");
    assert_eq!(f.name(), b"Subject");
    assert!(f.text.ends_with(b"\n"));
}

#[test]
fn from_line() {
    let f = Field::new(b"From user@host Mon Jan 1 00:00:00 2024\n".to_vec())
        .unwrap();
    assert!(f.is_from_line());
}

#[test]
fn name_matches() {
    let f = Field::new(b"Subject: Test\n".to_vec()).unwrap();
    assert!(f.name_matches(b"Subject"));
    assert!(f.name_matches(b"subject"));
    assert!(f.name_matches(b"Subj"));
    assert!(!f.name_matches(b"From"));
}

#[test]
fn field_rename() {
    let mut f = Field::new(b"Subject: Test\n".to_vec()).unwrap();
    f.rename(b"X-Subject:");
    assert_eq!(f.name(), b"X-Subject");
    assert!(f.text.starts_with(b"X-Subject:"));
}

#[test]
fn read_header_basic() {
    let input = b"From: user@host\nSubject: Test\n\nBody here\n";
    let (fields, body) = read_headers(&input[..]).unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(body, b"Body here\n");
}

#[test]
fn keep_first() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Received: first\n".to_vec()).unwrap());
    list.push(Field::new(b"Subject: Test\n".to_vec()).unwrap());
    list.push(Field::new(b"Received: second\n".to_vec()).unwrap());
    list.keep_first(b"Received");
    assert_eq!(list.len(), 2);
    assert!(list.find(b"Received").unwrap().value().ends_with(b"first"));
}

#[test]
fn keep_last() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Received: first\n".to_vec()).unwrap());
    list.push(Field::new(b"Subject: Test\n".to_vec()).unwrap());
    list.push(Field::new(b"Received: second\n".to_vec()).unwrap());
    list.keep_last(b"Received");
    assert_eq!(list.len(), 2);
    assert!(list.find(b"Received").unwrap().value().ends_with(b"second"));
}

#[test]
fn zap_whitespace_adds_space() {
    let mut f = Field::new(b"Subject:NoSpace\n".to_vec()).unwrap();
    let modified = f.zap_whitespace();
    assert!(modified);
    assert_eq!(&f.text, b"Subject: NoSpace\n");
}

#[test]
fn zap_whitespace_already_has_space() {
    let mut f = Field::new(b"Subject: HasSpace\n".to_vec()).unwrap();
    let modified = f.zap_whitespace();
    assert!(!modified);
}

#[test]
fn is_empty_value_true() {
    // Truly empty (no value at all)
    let f = Field::new(b"X-Empty:\n".to_vec()).unwrap();
    assert!(f.is_empty_value());
}

#[test]
fn is_empty_value_whitespace() {
    // Whitespace-only is NOT empty (matches procmail)
    let f = Field::new(b"X-Empty:  \t \n".to_vec()).unwrap();
    assert!(!f.is_empty_value());
}

#[test]
fn is_empty_value_false() {
    let f = Field::new(b"X-Val: x\n".to_vec()).unwrap();
    assert!(!f.is_empty_value());
}

#[test]
fn zap_removes_empty() {
    // Only truly empty fields are removed, not whitespace-only
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: Test\n".to_vec()).unwrap());
    list.push(Field::new(b"X-Empty:\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    list.zap_whitespace();
    assert_eq!(list.len(), 2);
    assert!(list.find(b"X-Empty").is_none());
}
