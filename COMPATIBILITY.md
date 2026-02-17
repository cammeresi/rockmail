# Rockmail Compatibility Notes

This file documents intentional differences from procmail behavior.

## Rockmail command line arguments

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

Rockmail uses `\b` (zero-width word boundary) for both. This means:
- The boundary character is not consumed/included in the match
- No distinction between word-start vs word-end

Example where behavior differs:
- Pattern `\<word\>` against "a word here"
  - Procmail: matches " word " (spaces consumed)
  - Rockmail: matches "word" (zero-width boundaries)

This difference affects what gets captured with `\/` but the match
success/failure is the same in typical usage.

## Double Caret in Character Classes

The `^^` anchor syntax is not character-class-aware. A pattern like `[^^]`
(intended as a negated character class containing `^`) may be misinterpreted.
Workaround: use `[^\\^]` or reorder the class contents.

## mailstat: Maildir only

Rockmail's `mailstat` currently assumes Maildir layout. Folder paths in the
log are normalized by stripping the last two path components (e.g.
`/home/user/Maildir/new/1234567890.host` becomes `/home/user/Maildir`).
mbox-style paths (a single file) are not specially handled.

## mailstat: ~/.mailstatrc

Procmail's `mailstat` has no configuration file. Rockmail adds support for
`~/.mailstatrc` with the following commands:

- `ignore <folder>` — Exclude a folder from the summary output. Suppressed
  by the `-z` flag.
- `date_format <spec>` — Override the date format used in the "No mail
  arrived since ..." message. The spec uses strftime syntax
  (e.g. `%e %b, %H:%M`).  Not affected by `-z`.

The default date format is `%e %b, %H:%M`.

## formail removed

Formail has been removed due to security issues.  Procmail idioms tend to
pass untrusted input through the shell when piping to formail, which
creates command injection risks.  The header manipulation and duplicate
detection functions that formail provided are available natively via the
`@I`/`@i`/`@a`/`@A` header ops and `@D` duplicate detection described in
`ENHANCEMENTS.md`.

## Comsat/Biff Notifications

Procmail can notify the biff service when mail is delivered, allowing
terminals to display "You have new mail" messages. The `COMSAT` variable
controls this.

Rockmail does not support comsat notifications. The `COMSAT` variable
is ignored. This feature is rarely used on modern systems.

## TIMEOUT: SIGKILL escalation

Procmail sends only SIGTERM when a child process exceeds the TIMEOUT.
Rockmail sends SIGTERM, waits 1 second, then sends SIGKILL if the
process is still running. This ensures hung processes are cleaned up
but gives less time for graceful shutdown.

## NFS atime hack

Procmail has a `NO_NFS_ATIME_HACK` guard that writes the first byte of
an mbox, checks if `atime == mtime`, and sleeps one second if needed.
This tricks NFS into cache invalidation so other processes see new mail.
Rockmail does not implement this.  NFS mbox delivery is increasingly rare.

## Signal handling

Procmail uses SIGUSR1 to toggle verbose mode at runtime and SIGUSR2 to
terminate the current child process. Rockmail does not act on these
signals.

Procmail also uses per-signal exit codes when terminated: SIGTERM exits
with `EX_TEMPFAIL` (75), SIGHUP/SIGINT with `EX_CANTCREAT` (73), and
SIGQUIT with its own handler. Rockmail uses a single exit flag for all
termination signals.

Procmail's `qsignal` wrapper preserves pre-existing `SIG_IGN` dispositions
when installing handlers.  Rockmail installs handlers unconditionally.

## /etc/procmail.conf configuration file

Although this file is undocumented, procmail will read sitewide configuration,
but not rules, from this file.  Rockmail ignores this file.

## Exit codes

Procmail uses specific sysexits codes in situations that rockmail handles
differently or does not implement:

- `EX_NOUSER` (67) and `EX_NOPERM` (77) are returned by procmail's `-d`
  delivery mode, which rockmail does not support.
- `EX_OSFILE` (72) is returned when `/dev/null` cannot be opened at
  startup; rockmail does not open `/dev/null` at startup.
- Suspicious rcfiles (wrong owner, world writable) cause procmail to
  silently skip the rcfile and fall through to default delivery.
  Rockmail treats these as fatal errors.

In all cases, delivery failures produce `EX_CANTCREAT` (73), or
`EX_TEMPFAIL` (75) with the `-t` flag, matching procmail.
