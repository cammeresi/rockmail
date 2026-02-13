# Sender detection is severely incomplete

## Component
`src/bin/formail/main.rs`

## Severity
High

## Description

Procmail's `getsender()` (formail.c lines 269-304) implements:

- RFC 822 address brackets and comment handling
- Bangpath/UUCP address extraction
- "remote from" and "forwarded by" routing extraction
- Address quality scoring with penalties for:
  - User-only addresses
  - .UUCP addresses
  - user@host without domain
  - Bangpath routes
- Resent-* header priority weighting
- Return-Path nil value (`<>`) handling
- Multiple From_ lines

Corpmail's `find_sender()` is a simple priority list lookup that
handles none of the above.

## Impact

Auto-reply generation (`-r` flag) selects wrong sender in complex
routing scenarios.
