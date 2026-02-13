# -f with '-' argument for timestamp refresh not implemented

Procmail treats `-f -` specially: instead of setting the envelope sender, it
refreshes the From_ line timestamp to the current time (REFRESH_TIME). This
is not implemented; `-f -` currently sets the sender to "-".
