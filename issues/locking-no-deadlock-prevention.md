# No deadlock prevention (globlock check)

## Component
`src/locking/`

## Severity
Low

## Description

Procmail tracks a global lock (`globlock`) and rejects attempts
to lock the same file twice (locking.c:121-124).  Corpmail has
no equivalent check, so the recipe engine could deadlock if it
tries to lock the same file recursively.
