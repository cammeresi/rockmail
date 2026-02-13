# Field name matching is too permissive (prefix match)

## Component
`src/formail/field.rs`

## Severity
Low

## Description

`Field::name_matches()` allows prefix matching: "Subj" matches
"Subject".  Procmail uses exact field name matching in most contexts.

This means `-u Subj` would keep the first field whose name starts
with "Subj", which is confusing and differs from procmail.

## Location
`src/formail/field.rs` line 108-116.
