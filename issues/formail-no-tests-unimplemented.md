# Formail: missing test coverage

## Component
`src/bin/formail/`, `tests/formail.rs`, `tests/formail_gold.rs`

## Severity
Low

## Description

Missing test coverage for:

- Malformed headers with control characters (implemented, zero tests)
- Very long continuation lines (>1000 chars; existing test only covers 59 chars)
- Binary data in header values (only non-ASCII Subject tested, not full binary range)

### Resolved

The following items previously listed here now have test coverage:

- Content-Length header processing (gold: `split_content_length`)
- Digest header detection (gold: `split_digest`)
- Article header detection / Path: (gold: `sender_path_only`)
- Complex sender address parsing (unit + gold tests)
- Multiple From_ lines handling (unit + gold tests)
- Return-Path nil value handling (unit + gold tests)
