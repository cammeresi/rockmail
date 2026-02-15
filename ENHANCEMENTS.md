# Rockmail Enhancements

Features that extend beyond procmail's rcfile syntax.

## Variable Regex Substitution (`=~`)

Apply a regex substitution to a variable's current value, without
invoking a shell.

### Syntax

```
VAR =~ s/pattern/replacement/flags
```

- **Delimiter** — any non-alphanumeric character (e.g. `/`, `|`, `#`).
- **Pattern** — Rust `regex` crate syntax.
- **Replacement** — `$1`, `$2`, etc. for capture groups.
- **Flags** — `g` (global), `i` (case-insensitive).

`$VAR` references in pattern and replacement are expanded before use.

### Examples

```
SUBJECT =~ s/RE\?: /Re: /g
SUBJECT =~ s/^(\[[^]]*\] |Re: )*//
ADDR =~ s/^<(.*)>$/$1/
NAME =~ s|/|_|g
```
