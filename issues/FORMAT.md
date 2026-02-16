# Issue file format

Filename: `component-short-description.md`

```markdown
# Title

Severity: low | medium | high

Description of the issue.

## Location

- `src/path/file.rs:123` (`function_name`)

## Suggested fix

How to fix it.
```

Severity levels:

- **high** — exploitable via untrusted input, or crash/panic in normal operation
- **medium** — defensive gap that could become exploitable, or footgun for future callers
- **low** — code smell, matches procmail behavior, or only triggered in unusual environments
