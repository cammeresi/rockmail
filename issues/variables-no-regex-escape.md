# $\name regex-escaped variable expansion not implemented

Procmail supports `$\name` which expands a variable and escapes all regex
special characters in the result, making it safe to embed literal strings in
patterns. This is not implemented.
