# Builtin variables not initialized with defaults

## Component
`src/variables/builtins.rs`, `src/engine/mod.rs`

## Severity
High

## Description

Procmail's `initdefenv()` (variables.c:226-234) presets all
builtin variables into the environment during initialization:
SHELLMETAS, LOCKEXT, MSGPREFIX, SHELLFLAGS, SENDMAIL,
SENDMAILFLAGS, PROCMAIL_VERSION, and all numeric defaults.

Corpmail declares these as constants in `builtins.rs` but never
initializes them in the environment.  Rcfiles that rely on these
presets being available will fail.

Example that breaks:

    :0
    | $SENDMAIL -t

SENDMAIL is undefined, producing an empty pipe command.
