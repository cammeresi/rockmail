# Rockmail Enhancements

Features that extend beyond procmail's rcfile syntax.

## Pretty-Printed Errors

Rcfile parse errors are rendered with context via miette when stderr is a
terminal.  Run rockmail interactively to see annotated source spans,
underlines, and color.

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

## Native Header Manipulation (`@`)

Modify headers on the in-flight message without forking formail or sed.
The operation letter mirrors formail's flags.

### Syntax

```
@X Header: value
```

The value undergoes `$VAR` expansion but never touches a shell.

### Operations

| Syntax | Formail equiv | Meaning |
|--------|---------------|---------|
| `@I Header: val` | `formail -I "Header: val"` | Delete all matching, then insert |
| `@i Header: val` | `formail -i "Header: val"` | Rename existing to `Old-Header:`, insert new |
| `@a Header: val` | `formail -a "Header: val"` | Add only if header not present |
| `@A Header: val` | `formail -A "Header: val"` | Always add (append) |

### Examples

```
@I Subject: $SUBJECT
@a Lines: $LINES
@A X-Processed: yes
@i Subject: [list] $SUBJECT
```

## Native Duplicate Detection (`@D`)

Check the Message-ID against a cache file and set `DUPLICATE=yes` if the
message has been seen before.  This replaces the common procmail idiom of
piping through `formail -D` but without forking a subprocess.

### Syntax

```
@D <maxlen> <cachefile>
```

Both arguments undergo `$VAR` expansion.  `maxlen` is the maximum cache
size in bytes; `cachefile` is the path to the circular cache.

### Example

```
:0 Wh:
@D 8192 .msgid.cache

:0
* DUPLICATE ?? yes
/dev/null
```

There is no sender-based (`-r`) equivalent — only Message-ID detection.

## Non-ASCII Header Decoding During Matching

Rockmail decodes RFC 2047 encoded words (`=?charset?B?...?=` and
`=?charset?Q?...?=`) in mail headers during condition matching. This means
regex patterns can match the decoded text directly — e.g. a pattern for
"café" will match `=?UTF-8?Q?caf=C3=A9?=`. Procmail matches headers in
raw encoded form only.

The `@I`, `@i`, `@a`, and `@A` header ops automatically encode non-ASCII
values as RFC 2047 encoded words when inserting headers.
