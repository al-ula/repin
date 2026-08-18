# Experiment Result: S1 — redb store adapter

```text
Status: pending
Lifecycle stage: experimentation
Experiment specification: ../storage.md#3-experiment-s1-redb-store-adapter
Overall outcome: not run
```

## Result

No S1 run has been retained. The specification includes the owner-claim and
canonical graph schema, versioned key encoding, single-writer behavior,
durability/crash points, writer-lock cases, reopen/rebuild behavior, and
high-fan-in/file-replacement fixtures.

## Recommendation

Keep redb 4.1.0 as an evidence pin only. Do not accept it as the authoritative
store until the crash, writer-exclusion, owner-removal, migration, and reopen
cases pass without weakening the portable `Store` contract.

## Required evidence

- acknowledged commits reopen as a complete graph or a documented permitted
  commit-tail loss;
- owner-scoped removal never deletes another producer's claim;
- exactly one writer is observable and recoverable after process death;
- migration/rebuild leaves a valid revision or a resumable recovery marker; and
- all required indexes remain deterministic under high fan-in and replacement.
