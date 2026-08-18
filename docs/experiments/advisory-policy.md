# Q-007 — Advisory response policy

This policy applies to the disposable foundation spike release-tool evidence
pass. It is a provisional experiment policy; it does not accept a dependency
or define a production release gate while the F7 decision remains deferred.

## Finding handling

- Vulnerability and unsoundness findings fail the policy gate by default.
- Unmaintained and notice findings are retained and reported as warnings; they
  do not block the release evidence pass.
- Unknown finding kinds fail closed.
- `cargo-audit` and `cargo-deny` ignore lists remain empty. The wrapper in
  [`q_policy.py`](foundation_spike/scripts/q_policy.py) evaluates only the
  explicit, machine-checkable exception records.

## Temporary exceptions

The only exception source is
[`advisory-exceptions.toml`](foundation_spike/advisory-exceptions.toml).
Every record must contain an exact advisory ID, package, and version together
with `owner`, `rationale`, `remediation_issue`, `compensating_control`,
`created`, and `expires`. Wildcards and package-wide exceptions are rejected.

An exception may last at most 30 days, may not be ownerless or malformed, and
fails validation after its expiry date. The current file intentionally has no
exceptions. Unit tests cover vulnerability, unsoundness, unmaintained, notice,
valid exact exceptions, expired exceptions, and overlong exceptions:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The real negative fixture is `time = 0.1.45`, which the retained audit JSON
reports as `RUSTSEC-2020-0071`; the wrapper classifies it as blocking. The
normalized policy result is retained in
[`audit-time-advisory-policy.json`](results/raw/q-release-tools-20260818/artifacts/audit-time-advisory-policy.json).

The full Q evidence report remains
[`decision_status: deferred`](results/raw/q-release-tools-20260818/report.json):
it records a reviewed provisional policy, not final dependency acceptance.
