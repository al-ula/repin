# ADR-012: Use exact Rust vector search as the I5 baseline

```text
Status: accepted future implementation choice; implementation deferred to I5
Date: 2026-08-19
Decision type: optional semantic retrieval adapter
Builds on: ADR-007 and ADR-009
```

## Decision

When semantic retrieval enters I5, Repin starts with exact vector search rather
than an approximate-nearest-neighbour dependency. Embeddings and their indexed
metadata are stored in derived SQLite tables. The adapter filters rows in SQL,
streams vectors through Rust distance computation, retains a bounded top-k
heap, and applies a stable `VectorKey` tie-break.

Vector generation and updates remain asynchronous. The graph revision never
waits for embeddings, and the semantic revision advances in a later
transaction. The graph remains authoritative and stale hits are dropped.

The detailed profile, acceptance criteria, and ANN escalation path are recorded
in the [Rust-friendly vector proposal](../proposals/vector-search-rust-friendly.md).

## Rationale

The exact baseline has no new native or ANN dependency, gives exact recall,
makes keyed deletion and reopen behavior straightforward, and provides a
reference oracle if approximate search is later justified. Its linear scan
cost is acceptable as a starting hypothesis because semantic retrieval is
optional and I5 includes fixed-corpus measurement.

## Consequences

- I5 has a selected starting adapter without pulling vector work into the
  deterministic implementation milestones.
- The minimum `.repin` layout needs no vector sidecar directory.
- Dimension and metric are fixed by the configured embedding profile and
  enforced by the adapter.
- If exact search misses the accepted I5 resource budget, compare a pure-Rust
  ANN implementation against the exact recall oracle before accepting it.
- USearch and libSQL-native vectors are not part of the selected profile.

## Reopen triggers

Reopen the adapter choice if filtered exact search misses the I5 latency or
memory budget, SQLite row streaming becomes the dominant semantic-query cost,
or a required semantic scale cannot be served within bounded deadlines.

## Not decided

This ADR does not select an embedding provider/model, enable semantic retrieval
by default, or move vector implementation before I5.
