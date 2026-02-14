# Gold test for pipe capture

## Component
`tests/rockmail_gold.rs`

## Severity
Low

## Description

No gold test compares rockmail vs procmail for pipe capture (`VAR=| cmd`)
syntax. Add a gold test that uses pipe capture to assign a variable and
then uses that variable in a subsequent recipe.
