# Experiment Result: F2 — Filesystem discovery and containment

```text
Status: complete (Tier 1 evidence retained; overall inconclusive)
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#3-f2-filesystem-discovery-and-containment
Run ID: foundation-tier1-20260818
Overall outcome: inconclusive
```

## Result

The Tier 1 fixture selected the expected four files using `ignore` 0.4.31 and
`globset` 0.4.19. Capability-relative reads accepted the in-root file and
rejected traversal and the escaping symlink. The existing F-008 report remains
the stronger Linux race evidence: 200 component/final-symlink attempts returned
no out-of-root bytes.

The maintained content comparator was run with `infer` 0.19.0 in the
feature-enabled configuration. It returned no MIME classification for the
small text, binary, invalid-byte, or minified fixtures. The in-house policy
classified NUL-containing and invalid-UTF-8 inputs as binary and the reviewed
text inputs as text. No hard blocker was observed; the missing corpus and
Linux PoC cases are evidence gaps, not normative-contract failures.

## Provisional recommendation (decision deferred)

Retain capability-relative opens, `cap-std`, `ignore`, and `globset` as
provisional candidates for the next evidence pass. Treat the `infer` result as
an observation rather than a content-policy selection; F-009 needs a larger
labeled corpus and an explicit false-positive policy.

This recommendation is recorded for later plan finalization. The experiment
does not accept or reject a candidate or select an implementation default.

## Required follow-up

- complete the path manifest, cycle, encoding, case, limit, and mutation cases;
- compare the two sniff policies on the reviewed labeled corpus; and
- defer additional-platform containment/build validation until the fully
  featured Linux PoC is complete.

## Evidence

- [feature-run batch report](raw/foundation-tier1-features-20260818/batch.json)
- [feature-run F2 report](raw/foundation-tier1-features-20260818/F2-report.json)
- [F-008 Linux race report](f008-root-capability.md)
- [spike workspace](../foundation_spike/)
