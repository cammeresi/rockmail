# TRAP exit handler not implemented

Procmail executes the command stored in `TRAP` just before exiting, with
`$LASTFOLDER`, `$EXITCODE`, etc. available. Currently the variable is
declared but never executed on exit.
