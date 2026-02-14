# Add git hash to -v output in all binaries

## Component
All end user binaries

## Severity
Minor

## Description

Instead of outputting the version number like "v0.1.0", output like
"0.1.0 (aabbccdd)" using the first eight characters of the git hash.
