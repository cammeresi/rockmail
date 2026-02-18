# CLI arg parsing has limited coverage

Severity: low

Some tests exist in `src/bin/rockmail/tests.rs` but many argument paths
are uncovered.

Missing coverage:
- `-t` flag (tempfail on delivery error)
- `-o` override
- Invalid/conflicting arguments
