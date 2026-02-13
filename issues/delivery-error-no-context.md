# DeliveryError::Io loses path/operation context

## Component
`src/delivery/mod.rs`

## Severity
Low

## Description

`DeliveryError::Io(std::io::Error)` wraps the raw IO error without
recording which file or operation failed.  When delivery fails, the
error message shows the OS error but not the path or whether it was
a read, write, lock, or rename operation.

## Suggestion

Add path and operation context to the error variant, e.g.:

```rust
Io { source: std::io::Error, path: PathBuf, op: &'static str }
```
