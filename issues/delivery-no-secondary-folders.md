# Secondary/carbon-copy folder delivery not implemented

## Component
`src/delivery/`

## Severity
Moderate

## Description

Procmail supports delivering to multiple secondary folders via hard
linking (`mailfold.c:194,205-217,312-330`).  This allows creating
copies of a message across multiple folders efficiently.

Corpmail has no equivalent feature.  There is no way to specify
multiple delivery destinations for a single recipe action.

## Procmail reference
`mailfold.c` — linkstrstrfstrfd and related secondary folder logic.
