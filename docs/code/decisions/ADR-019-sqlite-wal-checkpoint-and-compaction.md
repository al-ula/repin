# ADR-019: SQLite post-batch WAL checkpointing and storage compaction

```text
Status: accepted contract and capability decision
Date: 2026-08-20
Decision type: storage lifecycle and disk footprint management
Builds on: ADR-009, ADR-015
```

## Decision

Repin enforces automatic write-ahead log (WAL) checkpointing and storage compaction upon the completion of batch indexing and file update transactions:

1. **Explicit Post-Batch WAL Checkpoint**: At the end of initial full-repository indexing (`repin index`) and bulk update commits (`repin update`), the engine automatically executes:
   ```sql
   PRAGMA wal_checkpoint(TRUNCATE);
   ```
   This flushes all committed WAL pages back into the primary `.repin/graph.sqlite3` database file and truncates `graph.sqlite3-wal` to 0 bytes.
2. **Store Port Maintenance Contract**: The `Store` trait exposes a synchronous `checkpoint(&self) -> Result<(), StoreError>` method, allowing callers (CLI, daemon, batch indexer) to initiate explicit database compaction without needing direct SQLite connection handles.
3. **Graceful Handling of Open Read Views**: If concurrent readers hold open read locks during checkpointing, `TRUNCATE` mode safely checkpoints as many frames as possible without blocking active readers or violating serializable snapshot isolation.

## Rationale

Empirical benchmarks on medium-to-large codebases revealed that uncheckpointed SQLite WAL logs often account for >60% of total `.repin` storage footprint (e.g. 4.44 MB of WAL on a 2.68 MB base database). Automatically executing `PRAGMA wal_checkpoint(TRUNCATE);` at the conclusion of batch writes reduces on-disk storage by more than half, producing a compact, portable on-disk index.

## Consequences

- `Store` trait includes `checkpoint(&self) -> Result<(), StoreError>`.
- `SqliteStore` implements WAL truncation and compaction routines.
- `Engine` and CLI indexing commands invoke checkpointing automatically on batch completions.
- Total `.repin` directory size drops significantly post-index without requiring manual maintenance scripts.

## Required implementation validation

1. Checkpointing successfully truncates `graph.sqlite3-wal` after repository indexing.
2. Read operations continue functioning uninterrupted before and after checkpointing.
3. In-memory databases handle checkpoint invocations cleanly without errors.

## Reopen triggers

Reopen this decision if synchronous `TRUNCATE` checkpoints introduce unacceptable pause times on extremely large multi-gigabyte databases under continuous write load.
