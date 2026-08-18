# Experiment Result: S3 — Vector adapter

```text
Status: deferred
Lifecycle stage: planning
Experiment specification: ../storage.md#5-experiment-s3-usearch-vector-adapter
Overall outcome: intentionally not run
```

## Result

S3 is intentionally not run at Stage 2 exit. The planning decision records
that vector search is optional, deterministic capabilities must work without a
`Vector` adapter, and the experiment reopens only when semantic or hybrid
retrieval enters an implementation milestone.

## Recommendation

Defer execution. Do not describe USearch as accepted, do not make a vector
index a release prerequisite, and do not invent S3 measurements. At reopen,
run USearch and at least one shortlist alternative against the unchanged
`Vector` contract and identical cases.

## Required evidence at reopen

- deletion and supersession never leak stale vector hits;
- dimension, metric, filtering, truncation, and corruption behavior are
  explicit;
- rebuild/discard cannot affect deterministic graph or lexical capabilities;
- native distribution and platform support are acceptable; and
- at least one alternative candidate is measured in the same run group.
