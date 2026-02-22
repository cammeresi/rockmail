# ROCKMAIL — Rust Only Collator and Keeper of MAIL

[![CI](https://github.com/cammeresi/rockmail/workflows/Rust/badge.svg)](https://github.com/cammeresi/rockmail/actions)
[![codecov](https://codecov.io/gh/cammeresi/rockmail/branch/master/graph/badge.svg)](https://codecov.io/gh/cammeresi/rockmail)

## About

Rockmail is a translation of [Procmail][procmail] into Rust.  Procmail is
a Mail Delivery Agent (MDA) for filtering, sorting, and delivering mail
on Unix systems.

The goal is a near-100% backward-compatible drop-in replacement.
Most existing `.procmailrc` files should work without modification.
A small number of features are deliberately not implemented and a small
number of differences exist.  They are documented in `COMPATIBILITY.md`.

Some extensions beyond Procmail are also available; see `ENHANCEMENTS.md`.

[procmail]: https://en.wikipedia.org/wiki/Procmail

## Implementation

This software has been translated from C into Rust using AI, but it is
not "vibe coded."  The AI worked from the original C source code, and a
human operator was present at all times and provided guidance as needed.
All of the code has been reviewed by a human, though that does not mean
it is correct; several defects were initially unnoticed by the human.

Additionally, large parts of the code have been tested only by machine,
not by humans in actual usage.  Sorry, but I do not use every feature that
Procmail has, many of which I did not even know about.  Some features
are not even documented.  I was very inclusive when porting and only
omitted features with security problems or that I suspected had not been
used by anyone anywhere for multiple decades.

I cannot test all of that myself, but see notes on testing below to read
how I have attempted to bridge the gap.

I am not going to tell you what is tested by my personal setup and what
is not; it is better that you pay very close attention if you install
this software.  Better would be to audit it yourself!

## Installation

Build it, then feed mail into it, just as you would procmail.

If you need further instructions, you probably should not use this
software.

THIS SOFTWARE HAS SEEN LIMITED PRODUCTION USE, AND IT SHOULD BE
DEPLOYED CAREFULLY.  DO NOT SWAP IT IN AS YOUR PROCMAIL REPLACEMENT
WITHOUT TESTING!

DO NOT INSTALL THIS SOFTWARE SETUID ROOT!  Root privileges are not needed!
Root privileges will not be dropped!  Rockmail expects that the MTA will
setuid to the destination user before starting delivery.

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

Test coverage exceeds 97% as of 2026-02-21 as measured by `cargo
llvm-cov`.  Most of the coverage gaps are in error situations that are
difficult to test in automation.

On 2026-02-21, the following interesting statistics were observed:

- 12K lines of C in the original Procmail (as of version 3.24)
- 21K lines of Rust code
- of which 13K lines are tests (compared to zero in Procmail)
- for 8K lines of net software

(But remember what the original code contains, to wit:  custom memory
management, string manipulation, a regular expression engine, etc.
Although once necessary, all of that code can now, 35 years later,
be left behind thanks to the Rust standard library and crate ecosystem.)

On 2026-02-21, the 933 tests that were present comprised:

- 769 unit tests
- 48 integration tests
- 112 gold tests
- 4 regression tests

"Gold" means an integration test that compares Rockmail output to
Procmail output (the gold standard) and ensures they are byte identical.

During initial development, gold testing revealed one minor bug in procmail!

If running the tests yourself, it would be well to:

- Use `cargo nextest run` instead of `cargo test`
- Compile a custom procmail with the ssleep set to zero in `mailfold.c`

These two steps speed up the tests by a factor of five.

## Licensing

Most of this repository is a translation of procmail's C code into Rust.
That translated code is a work derived from procmail, so the license for
that code and the default license for code in this repository is GPLv2,
the same as for Procmail.

Some smaller parts of the code are NOT derivatives and are therefore subject
to a license chosen by me, which is the included 3-clause BSD license.

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

#### src/locking/flock/

This module is an alternative locking implementation designed for local
filesystems only, instead of procmail's NFS-oriented algorithm.

## Author

Sidney Cammeresi <sac@cheesecake.org>

