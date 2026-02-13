# HeaderIter doesn't handle >From_ continuation lines

## Component
`src/mail/message.rs`

## Severity
Low

## Description

`HeaderIter` skips "From " lines but does not skip or specially
handle ">From " lines that appear in forwarded mail headers.
If a ">From " line appears in the header section, it will be
treated as a regular (malformed) header and silently skipped
only because it lacks a colon.

The behavior happens to be correct by accident but is not
explicitly tested.
