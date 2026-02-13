# Dir (//) delivery missing double-newline enforcement

## Component
`src/delivery/maildir.rs`

## Severity
High

## Description

Procmail defines `ft_forceblank(type)` as `((type)!=ft_MAILDIR)`,
meaning all delivery types except Maildir must force a trailing blank
line (`\n\n`).  This includes Dir (`//`) delivery.

Corpmail's `deliver_dir()` calls `write_msg()` which only ensures a
single trailing newline, not the double newline required for proper
message separation.

## Procmail reference
`mailfold.c` — `ft_forceblank` macro and its usage in write logic.
