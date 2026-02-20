# ROCKMAIL — Rust Only Collator and Keeper of MAIL

## About

Rockmail is a rewrite of [Procmail](https://en.wikipedia.org/wiki/Procmail)
in Rust.  Procmail is a Mail Delivery Agent (MDA) for filtering, sorting,
and delivering mail on Unix systems.

The goal is a near-100% backward-compatible drop-in replacement.
Most existing `.procmailrc` files should work without modification.
A small number of features are deliberately not implemented and a small
number of differences exist.  They are documented in `COMPATIBILITY.md`.

Some extensions beyond Procmail are also available; see `ENHANCEMENTS.md`.

THIS SOFTWARE HAS SEEN LIMITED PRODUCTION USE, AND IT SHOULD BE
DEPLOYED CAREFULLY.  DO NOT SWAP IT IN AS YOUR PROCMAIL REPLACEMENT
WITHOUT TESTING!

## Differences from Procmail

Formail has been removed due to security issues.  Procmail idioms tend to
pass untrusted input through the shell when piping to formail, creating
command injection risks.  Its functionality is replaced by native rcfile
syntax (see below).

Notable extensions:

- **Header manipulation** (`@I`, `@i`, `@a`, `@A`) — modify headers on the
  in-flight message without forking a subprocess, replacing `formail -I`,
  `-i`, `-a`, `-A`.
- **Duplicate detection** (`@D`) — check Message-ID against a cache file,
  replacing `formail -D`.
- **Regex substitution** (`=~`) — apply `s/pattern/replacement/flags` to a
  variable without invoking a shell.
- **RFC 2047 decoding** — encoded headers are decoded during condition
  matching, so patterns can match the decoded text directly.  Manipulated
  headers are automatically RFC 2047 encoded if they contain non-ASCII.
- **Pretty-printed errors** — parse errors are rendered with source context
  and color via the `miette` crate if running in a terminal.

See `COMPATIBILITY.md` for the full list of intentional behavioral
differences.

## Testing and source code statistics

Test coverage exceeds 96% as of 2026-02-18.  Most of the coverage gaps are
in error situations that are difficult to test in automation.

On 2026-02-19, the following interesting statistics were observed:

- 12K lines of C in the original Procmail (as of version 3.24)
- 19K lines of Rust code
- of which 11K lines are tests (compared to zero in Procmail)
- for 8K lines of net software

(But remember what the original code contains, to wit:  custom memory
management, string manipulation, a regular expression engine, etc.
Although once necessary, all of that code can now, 35 years later,
be left behind thanks to the Rust standard library and crate ecosystem.)

The 860 tests that were present comprised:

- 725 unit tests
- 41 integration tests
- 90 gold tests
- 4 regression tests

"Gold" means an integration test that compares Rockmail output to Procmail
output (the gold standard) and ensures they are byte identical.

## Licensing

Most of this repository is a translation of procmail's C code into Rust.
That translated code is a work derived from procmail, so the license for
that code and the default license for code in this repository is GPLv2.

Some smaller parts of the code are NOT derivatives and are therefore subject
to a license chosen by me, which is the 3-clause BSD license.

### BSD-licensed components

#### src/bin/mailstat

Procmail contains a shell script with the same name as this binary, but
due to bugs in it, I wrote my own version in Python between 2000 and 2002
without making any reference to the original shell script.

This binary is a translation into Rust of that Python code, NOT of the
original shell script.  The original shell script was not used in any
way to produce this Rust code.

If the `nfs` feature is NOT used, this program may be used under the terms
of the included BSD license.  If `nfs` is enabled, then the compiled program
is infected by GPL restrictions and may be used only under GPL restrictions.

#### src/locking/flock.rs

This file is an alternative locking implementation designed for local
filesystems only, instead of procmail's NFS-oriented algorithm.

## Author

Sidney Cammeresi <sac@cheesecake.org>

