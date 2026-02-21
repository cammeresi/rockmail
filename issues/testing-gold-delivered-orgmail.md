# DELIVERED and ORGMAIL have no gold tests

Severity: medium

`DELIVERED=yes` suppresses default delivery. `ORGMAIL` is the fallback
when `DEFAULT` delivery fails. Neither is tested against procmail.

Missing coverage:
- Set `DELIVERED=yes` with no explicit delivery, verify no default delivery
- Set `DEFAULT` to an invalid path, verify fallback to `ORGMAIL`
- Set `DELIVERED=yes` after a recipe delivers, verify no double delivery

## Location

- `src/bin/rockmail/main.rs:398` (`DELIVERED` check)
- `src/bin/rockmail/main.rs:274,403` (`ORGMAIL` fallback)

## Suggested fix

Add gold tests for each scenario above.
