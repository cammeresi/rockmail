# Default umask not set to 077

## Component
`src/engine/mod.rs`

## Severity
High

## Description

Procmail sets `umask(077)` on startup (`INIT_UMASK` in `config.h:153`,
called at `procmail.c:227,481`).  Rockmail inherits the caller's umask
instead.

This affects delivered file permissions and also gates the
`UPDATE_MASK` chmod logic (`delivery-no-permission-update.md`).
