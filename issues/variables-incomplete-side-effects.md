# Incomplete magic variable side effects

## Component
`src/engine/mod.rs`, `src/variables/`

## Severity
Moderate

## Description

Procmail's `asenv()` (variables.c:353-442) triggers side effects
when certain variables are set.  Corpmail handles some but not all:

| Variable     | Procmail effect          | Corpmail |
|-------------|--------------------------|----------|
| LINEBUF     | Reallocate line buffer   | Missing  |
| MAILDIR     | chdir to directory       | Partial  |
| LOGFILE     | Open log file            | Partial  |
| LOG         | Append to logfile        | Partial  |
| DELIVERED   | Fake delivery status     | Missing  |
| EXITCODE    | Override exit code       | Missing  |
| SHIFT       | Shift argv               | Missing  |
| UMASK       | Set file umask           | Present  |
| HOST        | Validate hostname match  | Missing  |
| INCLUDERC   | Load rcfile              | Present  |
| SWITCHRC    | Switch rcfile            | Present  |
