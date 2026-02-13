# Formail: Resent-* reply-mode weights are dead code

## Component
`src/bin/formail/main.rs` — SEST table

## Severity
Low

## Description

The `wrrepl` field in the SEST sender-scoring table defines weights for
Resent-Reply-To, Resent-From, and Resent-Sender headers in reply mode.
These weights are populated but never read — only `wrepl` (envelope mode)
is used in `get_sender()`.  The field is marked `#[allow(dead_code)]`
with a TODO.

Procmail's formail uses `wrrepl` when invoked with `-r` (reply mode) to
prefer Resent-* headers over their non-Resent counterparts.
