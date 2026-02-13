# MH blank-line logic is buggy

## Component
`src/delivery/mh.rs`

## Severity
Moderate

## Description

At `mh.rs:64-67`, the code checks `!data.ends_with(b"\n\n")` and
adds a single `\n`.  The comment says "weirdly, procmail checks for
two then adds only one."

The logic is wrong: if the message ends with `X` (no newline at all),
it adds one `\n` producing `X\n` — but should produce `X\n\n` to
match procmail's `ft_forceblank` behavior for MH folders.

The correct behavior is to ensure the message ends with `\n\n` in
non-raw mode, adding one or two newlines as needed.

## Procmail reference
`mailfold.c:115-119` — blank line enforcement logic.
