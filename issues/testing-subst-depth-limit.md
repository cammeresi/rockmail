# Variable expansion depth limit is untested

Severity: low

`MAX_SUBST_DEPTH=32` exists for variable substitution but is never
exercised by tests.  The INCLUDERC recursion limit is tested separately
(`src/engine/tests.rs:932`) but variable substitution depth is not.
