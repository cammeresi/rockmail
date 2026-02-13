# Regex macros not tested with real procmailrc files

The `^TO_`, `^TO`, `^FROM_DAEMON`, and `^FROM_MAILER` macros have unit tests
but have not been tested end-to-end with actual procmailrc recipes that use
them. Gold tests with real-world patterns would catch expansion or matching
issues.
