# Invalid regex in conditions is untested

Severity: low

No test verifies what happens when a recipe condition contains an invalid
regex pattern.  The engine should handle compilation failure gracefully
rather than panicking.
