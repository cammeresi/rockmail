# ROCKMAIL — Rust Only Configurable Keeper of MAIL

## Licensing

Most of this repository is a translation of procmail's C code into Rust.
That translated code is a work derived from procmail, so the license for
that code and the default license for code in this repository is GPLv2.

Some smaller parts of the code are NOT derivatives and are therefore subject
to a license chosen by me, which is the 3-clause BSD license.

### BSD-licensed components

#### src/bin/mailstat

Procmail contains a shell script with the same name as this binary, but
due to bugs in it, I wrote my own version in Python between 2000 and 2002
without making any reference to the original shell script.

This binary is a translation into Rust of that Python code, NOT of the
original shell script.  The original shell script was not used in any
way to produce this Rust code.

If the `nfs` feature is NOT used, this program may be used under the terms
of the included BSD license.  If `nfs` is enabled, then the compiled program
is infected by GPL restrictions and may be used only under GPL restrictions.

#### src/locking/flock.rs

This file is an alternative locking implementation designed for local
filesystems only, instead of procmail's NFS-oriented algorithm.

#### src/util/mod.rs

This file encapsulates mainly a few constants for exit codes that procmail
returns, and numbers are not copyrightable.

## Author

Sidney Cammeresi <sac@cheesecake.org>

