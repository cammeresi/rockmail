# Backtick command substitution not implemented

Variable assignments with backticks (`FOO=\`command\``) store the literal
string instead of executing the command and capturing its output.

Procmail evaluates backticks at assignment time: `eputenv()` calls
`readparse()` which handles backticks by calling `fromprog()` to fork,
execute the command, and capture stdout. The entire mail message is passed
on stdin.

In rockmail, `parse_assignment` stores the value as-is and `expand()` only
handles `$var` substitution.
