# Trusted IDs for -f option not implemented

Procmail only allows certain trusted uids (root, daemon, mail) to use `-f` to
set the envelope sender. Other users get a warning and the flag is ignored.
Currently any user can use `-f` without restriction.
