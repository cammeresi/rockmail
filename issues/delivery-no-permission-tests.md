# No tests for file mode/permission handling on delivery

## Component
`src/delivery/`

## Severity
Low

## Description

No tests verify that delivered files and folders get correct
permissions.  Missing coverage:

- Mbox file permissions after creation and append
- Maildir subdirectory (tmp, new, cur) permissions
- MH folder and message file permissions
- Dir (directory) permissions
- Behavior under restrictive umask
