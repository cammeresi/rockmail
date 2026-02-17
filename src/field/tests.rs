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
fn rename_without_colon() {
    let mut f = Field::new(b"Subject: Test\n".to_vec()).unwrap();
    f.rename(b"X-Subject");
    assert_eq!(f.name(), b"X-Subject");
    assert_eq!(&f.text, b"X-Subject: Test\n");
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

#[test]
fn concatenate_folds() {
    let mut f = Field::new(b"Subject: line1\n\tline2\n".to_vec()).unwrap();
    f.concatenate();
    assert_eq!(&f.text, b"Subject: line1 \tline2\n");
}

#[test]
fn concatenate_skips_from_line() {
    let mut f =
        Field::new(b"From user@host Mon Jan 1 00:00:00 2024\n".to_vec())
            .unwrap();
    let orig = f.text.clone();
    f.concatenate();
    assert_eq!(f.text, orig);
}

#[test]
fn field_len() {
    let f = Field::new(b"Subject: Hi\n".to_vec()).unwrap();
    assert_eq!(f.len(), 12);
    assert!(!f.is_empty());
}

#[test]
fn find_mut_modifies() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: old\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    let f = list.find_mut(b"Subject").unwrap();
    f.rename(b"X-Subject:");
    assert_eq!(list[0].name(), b"X-Subject");
}

#[test]
fn remove_all_removes() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Received: a\n".to_vec()).unwrap());
    list.push(Field::new(b"Subject: x\n".to_vec()).unwrap());
    list.push(Field::new(b"Received: b\n".to_vec()).unwrap());
    list.remove_all(b"Received");
    assert_eq!(list.len(), 1);
    assert!(list.find(b"Received").is_none());
}

#[test]
fn rename_all_renames() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Received: a\n".to_vec()).unwrap());
    list.push(Field::new(b"Subject: x\n".to_vec()).unwrap());
    list.push(Field::new(b"Received: b\n".to_vec()).unwrap());
    list.rename_all(b"Received", b"X-Received:");
    assert!(list.find(b"Received").is_none());
    assert_eq!(list[0].name(), b"X-Received");
    assert_eq!(list[2].name(), b"X-Received");
}

#[test]
fn prepend_old_prefixes() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: x\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    list.prepend_old(b"Subject");
    assert_eq!(list[0].name(), b"Old-Subject");
    assert_eq!(list[1].name(), b"From");
}

#[test]
fn whitespace_before_colon() {
    let f = Field::new(b"Subject : Hello\n".to_vec()).unwrap();
    // "Subject " (8 bytes), colon at [8], name_len = 9
    assert_eq!(f.name_len, 9);
    // name() includes trailing whitespace before colon
    assert_eq!(f.name(), b"Subject ");
    assert_eq!(f.value(), b" Hello");
}

#[test]
fn whitespace_tab_before_colon() {
    let f = Field::new(b"Subject\t: Hello\n".to_vec()).unwrap();
    assert_eq!(f.value(), b" Hello");
}

#[test]
fn whitespace_multi_before_colon() {
    let f = Field::new(b"Subject \t : Hello\n".to_vec()).unwrap();
    assert_eq!(f.value(), b" Hello");
}

#[test]
fn whitespace_no_colon_after() {
    // Whitespace followed by non-colon → not a header
    assert!(Field::new(b"Not a header\n".to_vec()).is_none());
}

#[test]
fn write_to_output() {
    let mut list = FieldList::new();
    list.push(Field::new(b"From: a\n".to_vec()).unwrap());
    list.push(Field::new(b"To: b\n".to_vec()).unwrap());
    let mut buf = Vec::new();
    list.write_to(&mut buf).unwrap();
    assert_eq!(buf, b"From: a\nTo: b\n");
}
