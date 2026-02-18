# Parser `strip_comment()` has no isolated tests

Severity: low

`strip_comment()` is exercised indirectly through full rcfile parsing
tests but has no isolated unit tests covering edge cases like comments
at line start vs mid-line, multiple `#` characters, or `#` inside
quoted strings.
