# Weighted scoring has known algorithmic gaps

Severity: medium

30+ engine unit tests exist for weighted scoring, but the implementation
uses a closed-form formula instead of procmail's iterative approach.
Several behaviors are missing or incorrect.

Missing coverage (also missing implementation):
- Tail sums for convergent series (exponent < 1)
- Exit-code-based shell scoring
- Correct negated-weighted logic
- Empty-match handling
- Score clamping
