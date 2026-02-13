# No inline comment support

## Component
`src/config/parser.rs`

## Severity
Low

## Description

The parser only supports full-line comments starting with `#`.
Procmail supports inline comments, so a line like:

    MAILDIR=/var/mail  # user mail directory

is parsed by rockmail as setting MAILDIR to `/var/mail  # user mail directory`.

The test at `src/config/parser/tests.rs:116` explicitly documents this
as unsupported.

## Impact

Existing procmail rcfiles that use inline comments will silently
produce wrong variable values.
