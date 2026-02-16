# Regex extension interactions poorly tested

Severity: low

Individual procmail extensions are tested (`^^`, `\/`, `\<`, `\>`),
and `^^` + `\/` is tested together. But most pairwise combinations
are not:

- `\<` or `\>` with `\/` capture
- `\<` or `\>` with `^^` anchors
- Macros (`^TO_`, `^FROM_DAEMON`) with `\/` or `^^`
- Multiple `\/` in one pattern (only first should capture)
