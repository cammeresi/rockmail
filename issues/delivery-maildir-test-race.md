# Maildir test has FIXME for time-dependent race

## Component
`src/delivery/maildir/tests.rs`

## Severity
Low

## Description

At `maildir/tests.rs:65-66` there is an explicit FIXME:
"FIXME race, needs to pass time as a parameter."

The serial increment test is flaky if execution crosses a second
boundary, since the filename includes a timestamp.

## Fix

Inject a mock clock or accept a time parameter so the test is
deterministic.
