# Corpmail Compatibility Notes

This file documents intentional differences from procmail behavior.

## Corpmail command line arguments

### Unsupported arguments

- `-z` — LMTP server mode (RFC 2033)
- `-d` — delivery mode, which requires setuid root installation
- `-Y` — ignore Content-Length headers
- `-m` — mail filter mode, which is really a multi-user mode for sysadmins

### Minor variances

- `-p` — Environment is totally preserved, whereas procmail still filters evil variables and variables related to the dynamic linker.

## Regex Word Boundaries

Procmail's `\<` and `\>` consume a non-word character:
- `\<` matches a non-word char (or start of text) before a word
- `\>` matches a non-word char (or end of text) after a word

Corpmail uses `\b` (zero-width word boundary) for both. This means:
- The boundary character is not consumed/included in the match
- No distinction between word-start vs word-end

Example where behavior differs:
- Pattern `\<word\>` against "a word here"
  - Procmail: matches " word " (spaces consumed)
  - Corpmail: matches "word" (zero-width boundaries)

This difference affects what gets captured with `\/` but the match
success/failure is the same in typical usage.

## Double Caret in Character Classes

The `^^` anchor syntax is not character-class-aware. A pattern like `[^^]`
(intended as a negated character class containing `^`) may be misinterpreted.
Workaround: use `[^\\^]` or reorder the class contents.

## Trailing Backslash

A trailing backslash in a pattern (e.g., `foo\`) is treated as a literal
backslash, matching `foo\`. This matches procmail behavior.

## mailstat: Maildir only

Corpmail's `mailstat` currently assumes Maildir layout. Folder paths in the
log are normalized by stripping the last two path components (e.g.
`/home/user/Maildir/new/1234567890.host` becomes `/home/user/Maildir`).
mbox-style paths (a single file) are not specially handled.

## mailstat: ~/.mailstatrc

Procmail's `mailstat` has no configuration file. Corpmail adds support for
`~/.mailstatrc` with the following commands:

- `ignore <folder>` — Exclude a folder from the summary output. Suppressed
  by the `-z` flag.
- `date_format <spec>` — Override the date format used in the "No mail
  arrived since ..." message. The spec uses the `time` crate's v1 format
  description syntax (e.g. `[day] [month repr:short], [hour]:[minute]`).
  Not affected by `-z`.

The default date format is `[day] [month repr:short], [hour]:[minute]`.

## formail: -Y flag

The original formail's `-Y` flag (ignore Content-Length headers) is not
implemented. Content-Length headers are always ignored since corpmail uses
From_ line detection for message boundaries.

## formail: -q- syntax

The `-q-` syntax to disable quiet mode is not implemented. The `-q` flag
is accepted but has no effect (quiet mode is always on).

## Comsat/Biff Notifications

Procmail can notify the biff service when mail is delivered, allowing
terminals to display "You have new mail" messages. The `COMSAT` variable
controls this.

Corpmail does not support comsat notifications. The `COMSAT` variable
is ignored. This feature is rarely used on modern systems.

## formail: -s with command

The `-s` (split) flag does not support piping to a command. Only `-s`
without arguments is supported, which outputs split messages to stdout.

## formail: -B flag

Splitting BABYL format mailboxes is unsupported.

## /etc/procmail.conf configuration file

Although this file is undocumented, procmail will read sitewide configuration,
but not rules, from this file.  Corpmail ignores this file.
