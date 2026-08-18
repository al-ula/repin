# Experiment Result: S4 — Combined revision and recovery protocol

```text
Status: pending
Lifecycle stage: experimentation
Experiment specification: ../storage.md#6-experiment-s4-combined-revision-and-recovery-protocol
Overall outcome: not run
```

## Result

No S4 run has been retained. The specified matrix covers termination before
and after graph commits, lexical lag and repair, pending vector work, node
deletion, repeated reopen, and the vector-absent configuration used while S3
is deferred.

## Recommendation

Run S4 only after the S1/S2 adapters or faithful test doubles can produce the
required failure points. Do not finalize the cross-index recovery protocol
until every interruption either preserves the previous valid state or exposes
the new graph with honest derived-index lag and an idempotent repair path.

## Required evidence

- no acknowledged graph revision has an incomplete change summary;
- graph authority survives derived-index interruption;
- stale derived hits are dropped through graph validation;
- repeated recovery is idempotent; and
- the vector-absent path keeps deterministic revisions current and observable.
