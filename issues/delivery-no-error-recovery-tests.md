# No tests for delivery error recovery scenarios

## Component
`src/delivery/`

## Severity
High

## Description

No integration tests cover delivery failure and recovery paths:

- Partial write failure (ENOSPC, EDQUOT) and truncation recovery
- Large file write failures
- Maildir collision behavior under concurrent delivery
- `/dev/null` special case beyond unit tests
- File permission verification after delivery
- Lock acquisition failure handling
