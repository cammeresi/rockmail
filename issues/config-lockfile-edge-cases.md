# No lockfile parsing edge-case tests

## Component
`src/config/parser.rs`

## Severity
Low

## Description

Lockfile parsing handles explicit lockfiles (`:0:mylock`) and
auto-generated (`:0:`), but there are no tests for:

- Lockfile paths containing variables (`:0:/tmp/lock-$USER`)
- Special characters in lockfile paths
- Multiple colons that could confuse flag vs lockfile delimiter parsing
