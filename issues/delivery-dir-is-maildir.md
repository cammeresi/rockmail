# `//` suffix incorrectly treated as flat directory

## Problem

Rockmail treats a path ending in `//` as `FolderType::Dir`, delivering to a
flat directory with `msg.` prefix filenames. Procmail does not have this
behavior.

In procmail's `folderparse()` (`foldinfo.c:31`), `foo//` matches the
`ft_MAILDIR` branch (trailing `/`), then extra slashes are stripped. So `//`
is simply Maildir.

Procmail's `ft_DIR` only triggers when `folderparse` returns `ft_FILE` (no
suffix) and the path resolves to an existing directory at delivery time
(`foldertype` in `foldinfo.c:120-133`).

## Current behavior

- `inbox//` → `FolderType::Dir` → flat dir with `msg.<timestamp>` files
- `deliver_dir` in `maildir.rs` writes directly without `tmp/new/cur`

## Expected behavior

- `inbox//` → `FolderType::Maildir` → standard Maildir delivery
- `FolderType::Dir` should only be used for suffixless paths that are
  existing directories (runtime detection, not syntax-based)

## Files

- `src/delivery/mod.rs` — `FolderType::parse()` and `suffix()`
- `src/delivery/maildir.rs` — `deliver_dir`
