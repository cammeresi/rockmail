use super::*;

fn check_byte_len(list: &FieldList) {
    let sum: usize = list.iter().map(|f| f.len()).sum();
    assert_eq!(list.byte_len(), sum, "byte_len mismatch");
}

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
    assert!(f.as_bytes().ends_with(b"\n"));
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
    assert!(f.as_bytes().starts_with(b"X-Subject:"));
}

#[test]
fn rename_without_colon() {
    let mut f = Field::new(b"Subject: Test\n".to_vec()).unwrap();
    f.rename(b"X-Subject");
    assert_eq!(f.name(), b"X-Subject");
    assert_eq!(f.as_bytes(), b"X-Subject: Test\n");
}

#[test]
fn parse_bytes_basic() {
    let fields = parse_bytes(b"From: user@host\nSubject: Test\n");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name(), b"From");
    assert_eq!(fields[1].name(), b"Subject");
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
    check_byte_len(&list);
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
    check_byte_len(&list);
}

#[test]
fn zap_whitespace_adds_space() {
    let mut f = Field::new(b"Subject:NoSpace\n".to_vec()).unwrap();
    let modified = f.zap_whitespace();
    assert!(modified);
    assert_eq!(f.as_bytes(), b"Subject: NoSpace\n");
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
    check_byte_len(&list);
}

#[test]
fn concatenate_folds() {
    let mut f = Field::new(b"Subject: line1\n\tline2\n".to_vec()).unwrap();
    f.concatenate();
    assert_eq!(f.as_bytes(), b"Subject: line1 \tline2\n");
}

#[test]
fn concatenate_skips_from_line() {
    let mut f =
        Field::new(b"From user@host Mon Jan 1 00:00:00 2024\n".to_vec())
            .unwrap();
    let orig = f.as_bytes().to_vec();
    f.concatenate();
    assert_eq!(f.as_bytes(), orig);
}

#[test]
fn field_len() {
    let f = Field::new(b"Subject: Hi\n".to_vec()).unwrap();
    assert_eq!(f.len(), 12);
    assert!(!f.is_empty());
}

#[test]
fn rename_all_single() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: old\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    list.rename_all(b"Subject", b"X-Subject:");
    assert_eq!(list[0].name(), b"X-Subject");
    check_byte_len(&list);
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
    check_byte_len(&list);
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
    check_byte_len(&list);
}

#[test]
fn prepend_old_prefixes() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: x\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    list.prepend_old(b"Subject");
    assert_eq!(list[0].name(), b"Old-Subject");
    assert_eq!(list[1].name(), b"From");
    check_byte_len(&list);
}

#[test]
fn whitespace_before_colon() {
    let f = Field::new(b"Subject : Hello\n".to_vec()).unwrap();
    // "Subject " (8 bytes), colon at [8], name_len = 9
    assert_eq!(f.name_len(), 9);
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
fn parse_lax_skips_second_from() {
    let input = b"From user@a Mon Jan 1 00:00:00 2024\nSubject: Hi\nFrom user@b Mon Jan 1 00:00:00 2024\n";
    let fields = parse_bytes(input);
    assert_eq!(fields.len(), 2);
    assert!(fields[0].is_from_line());
    assert_eq!(fields[1].name(), b"Subject");
}

#[test]
fn parse_lax_no_trailing_newline() {
    let fields = parse_bytes(b"Subject: Hi");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].value(), b" Hi");
    assert!(fields[0].as_bytes().ends_with(b"\n"));
}

#[test]
fn write_to_output() {
    let mut list = FieldList::new();
    list.push(Field::new(b"From: a\n".to_vec()).unwrap());
    list.push(Field::new(b"To: b\n".to_vec()).unwrap());
    let mut buf = Vec::new();
    list.write_to(&mut buf).unwrap();
    assert_eq!(buf, b"From: a\nTo: b\n");
    check_byte_len(&list);
}

#[test]
fn byte_len_insert_remove_replace() {
    let mut list = FieldList::new();
    list.push(Field::new(b"From: a\n".to_vec()).unwrap());
    check_byte_len(&list);

    list.insert(0, Field::new(b"Subject: Hi\n".to_vec()).unwrap());
    assert_eq!(list[0].name(), b"Subject");
    check_byte_len(&list);

    list.remove(0);
    assert_eq!(list.len(), 1);
    check_byte_len(&list);

    list.replace_first(Field::new(b"To: b@c\n".to_vec()).unwrap());
    assert_eq!(list[0].name(), b"To");
    check_byte_len(&list);
}

#[test]
fn byte_len_concatenate_all() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: line1\n\tline2\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    let before = list.byte_len();
    list.concatenate_all();
    assert_eq!(list.byte_len(), before);
    check_byte_len(&list);
}

#[test]
fn unfold_to_bytes_matches_concatenate_all() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: line1\n\tline2\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    let unfolded = list.unfold_to_bytes();
    let mut cloned = list.clone();
    cloned.concatenate_all();
    let mut expected = Vec::new();
    cloned.write_to(&mut expected).unwrap();
    assert_eq!(unfolded, expected);
}

#[test]
fn unfold_to_bytes_skips_from_line() {
    let mut list = FieldList::new();
    list.push(
        Field::new(b"From user@host Mon Jan 1 00:00:00 2024\n".to_vec())
            .unwrap(),
    );
    list.push(Field::new(b"Subject: line1\n\tline2\n".to_vec()).unwrap());
    let out = list.unfold_to_bytes();
    assert!(out.starts_with(b"From user@host"));
    assert!(!out.windows(7).any(|w| w == b"line1\n\t"));
}

#[test]
fn remove_all_multiple() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Received: a\n".to_vec()).unwrap());
    list.push(Field::new(b"Subject: x\n".to_vec()).unwrap());
    list.push(Field::new(b"Received: b\n".to_vec()).unwrap());
    list.push(Field::new(b"From: user\n".to_vec()).unwrap());
    list.push(Field::new(b"Received: c\n".to_vec()).unwrap());
    list.remove_all(b"Received");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name(), b"Subject");
    assert_eq!(list[1].name(), b"From");
    check_byte_len(&list);
}

#[test]
fn insert_ordering() {
    let mut list = FieldList::new();
    list.push(Field::new(b"From: a\n".to_vec()).unwrap());
    list.push(Field::new(b"Subject: b\n".to_vec()).unwrap());
    list.push(Field::new(b"To: c\n".to_vec()).unwrap());
    list.insert(1, Field::new(b"Date: d\n".to_vec()).unwrap());
    assert_eq!(list.len(), 4);
    assert_eq!(list[0].name(), b"From");
    assert_eq!(list[1].name(), b"Date");
    assert_eq!(list[2].name(), b"Subject");
    assert_eq!(list[3].name(), b"To");
    check_byte_len(&list);
}

#[test]
fn push_with_continuation() {
    let mut list = FieldList::new();
    list.push(Field::new(b"Subject: a\n\tb\n".to_vec()).unwrap());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].as_bytes(), b"Subject: a\n\tb\n");
    assert_eq!(list[0].name(), b"Subject");
    check_byte_len(&list);
}

#[test]
fn parse_bytes_folded() {
    let input = b"Subject: line1\n\tcontinued\nFrom: user\n";
    let fields = parse_bytes(input);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].as_bytes(), b"Subject: line1\n\tcontinued\n");
    assert_eq!(fields[1].name(), b"From");
    check_byte_len(&fields);
}

#[test]
fn parse_bytes_multiple_continuations() {
    let input = b"Subject: a\n\tb\n c\nFrom: user\n";
    let fields = parse_bytes(input);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].as_bytes(), b"Subject: a\n\tb\n c\n");
    assert_eq!(fields[1].name(), b"From");
    check_byte_len(&fields);
}
