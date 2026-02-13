# LOGABSTRACT message summaries not implemented

Procmail logs a one-line summary of each delivered message (From, Subject,
Folder, Size) when `LOGABSTRACT` is set to "all" or "yes". Currently
`VAR_LOGABSTRACT` is declared and initialized but never consulted during
delivery.
