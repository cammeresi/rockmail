# Introduction

This crate is a translation to Rust of Procmail, a Mail Delivery Agent for
filtering, sorting, and delivering mail.

Procmail is a program that primarily filters incoming e-mails into different
folders.  This program is well known and is included in linux distributions, so
you can look up information about it on the internet if you need to.

The goal is a close to 100% backward-compatible drop-in replacement.  The
rcfile syntax and CLI must match Procmail exactly.  A few features are
deliberately not implemented; these features are documented in
`COMPATIBILITY.md`.

Some extensions have been implemented beyond what procmail supports.
Extensions are described in `ENHANCEMENTS.md`.

# Project Structure

## Binaries (`src/bin/`)

- `rockmail` — main MDA (drop-in for Procmail)
- `formail` — header manipulation and mailbox splitting
- `lockfile` — NFS-safe file locking utility
- `mailstat` — log statistics
- `rcparse` — debug utility for parsing rcfiles

## Library modules (`src/`)

- `config/` — rcfile parsing: parser, recipe, condition, action
- `engine/` — recipe evaluation loop, condition matching, scoring
- `delivery/` — mbox, maildir, MH, dir, pipe delivery
- `mail/` — message parsing, headers, From_ line handling
- `re/` — regex compiler/matcher with Procmail extensions (`^^`, `\/`, `\<`, `\>`)
- `variables/` — builtins, substitution (`$var`, `${var:-default}`)
- `locking/` — dotlock (NFS-safe) and flock
- `formail/` — formail-specific field manipulation
- `util/` — exit codes, error types, signal handling

## Tests

- Unit tests: colocated `tests.rs` files in each module
- Integration tests: `tests/rockmail.rs`, `tests/formail.rs`
- Gold tests: `tests/rockmail_gold.rs`, `tests/formail_gold.rs` —
  run both Rockmail and Procmail, compare output
- Regressions: `tests/regressions.rs`
- Common helpers: `tests/common/mod.rs`

## Other files

- `issues/` — known implementation gaps and bugs (one .md per issue)
- `COMPATIBILITY.md` — documented behavioral differences from Procmail

# Procmail Source

The original C source is at `/home/sac/src/procmail/src/`.  Key files:

- `procmail.c` — main program
- `misc.c` — condition evaluation, weighted scoring
- `mailfold.c` — folder delivery
- `locking.c` — lock logic
- `goodies.c` — variable substitution
- `regexp.c` — regex engine
- `pipes.c` — pipe handling
- `comsat.c` — biff notification
- `man/procmailrc.man` — rcfile format docs

# Known Major Gaps

Weighted scoring is the largest incomplete area.  See `issues/engine-*.md`
for details.  The implementation uses a closed-form formula instead of
Procmail's iterative match-by-match accumulation and is missing:
tail sums for convergent series, exit-code-based shell scoring,
correct negated-weighted logic, empty-match handling, and score clamping.

See the `issues/` directory for the full list.

# Instructions

When writing or reviewing Rust code, load skills related to writing
Rust code.  Also read skills related to organizing Rust code for context.

## Test style

Prefer `assert_eq!` over `match`/`panic!` in tests.  Construct expected
values by hand and compare with `assert_eq!`.  Add `PartialEq` (and
`Debug`) derives to types as needed to support this.

When tests need to destructure a specific enum variant, write a helper
that extracts it (panicking with a debug message on mismatch) and add a
`#[should_panic]` test for the helper.  See `recipe()` and `nested()` in
`src/config/parser/tests.rs` for examples.

