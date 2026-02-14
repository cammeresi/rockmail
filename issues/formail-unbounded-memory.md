# Unbounded memory accumulation in split mode

## Component
`src/bin/formail/main.rs`

## Severity
Low

## Description

In split mode (`-s`), formail accumulates entire messages in memory
via `extend_from_slice` (lines 980, 990, 996, 999, 1010) and
`read_to_end` (line 1121) without size limits.  A deliberately
oversized message on stdin could cause OOM.

This matches procmail's behavior — procmail also reads entire messages
into memory during splitting.  Imposing a size limit would break
backward compatibility.
