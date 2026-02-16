# Engine error paths mostly untested

Severity: medium

Tested: invalid regex pattern, unwritable delivery path.

Not tested:
- Recursion limit exceeded (`MAX_INCLUDE_DEPTH`)
- Lock acquisition failure
- Shell command timeout
- Logfile write failures
- Pipe spawn failures
