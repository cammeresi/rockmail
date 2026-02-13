# No streaming support for large messages

## Component
`src/mail/message.rs`

## Severity
Low

## Description

Procmail's `readmail()` reads messages in chunks using a line
buffer (`linebuf`), handles overflow conditions, and manages
the `themail` memblk structure for large messages.

Corpmail reads the entire message into memory at once with
`Message::parse()`.  There is no linebuf limit checking and no
streaming support for messages larger than available RAM.
