# No tests for unimplemented formail features

## Component
`src/bin/formail/`, `tests/formail.rs`

## Severity
Moderate

## Description

Zero test coverage for:

- Content-Length header processing
- Digest header detection
- Article header detection (USENET)
- Complex sender address parsing (bangpaths, "remote from")
- Resent-* header priority weighting
- Multiple From_ lines handling
- Return-Path nil value handling
- Malformed headers with control characters
- Very long continuation lines (>1000 chars)
- Binary data in header values
