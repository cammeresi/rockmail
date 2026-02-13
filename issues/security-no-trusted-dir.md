# /etc/procmailrcs/ trusted directory not implemented

Procmail checks `/etc/procmailrcs/` for per-user rcfiles when running as root
in delivery mode (`-d`). These files are trusted (owned by root) and bypass
normal security checks. Not implemented since `-d` mode is not supported.
