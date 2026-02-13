# TIMEOUT not enforced for child processes

Procmail uses `TIMEOUT` (default 960s) to kill child processes that run too
long via `SIGALRM`. Currently `VAR_TIMEOUT` is declared but the engine does
not set any alarm or enforce a time limit on pipe delivery or shell conditions.
