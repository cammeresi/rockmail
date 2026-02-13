# Formail security review

## Component
`src/bin/formail/` and all code called from there

## Severity
Critical

## Description

This program will process e-mail, which is untrusted input that could be
received from an attacker.  An aggressive security audit needs to be
conducted.

All possible security issues must be considered but especially:

- execution of arbitrary code
- execution of an unintended binary
- reading or writing an unintended file

