# ADR-014: Use a sparse-checkpoint line index

```text
Status: accepted implementation choice for the initial Linux PoC
Date: 2026-08-19
Decision type: byte-offset and public-position conversion
Builds on: ADR-006
```

## Decision

Repin builds one ephemeral line index per file content revision. It stores each
logical line's starting byte offset and sparse Unicode-scalar checkpoints only
for lines containing non-ASCII or invalid input. ASCII columns use the direct
byte delta fast path.

The initial private checkpoint stride is 128 bytes, aligned to decoded scalar
boundaries. Lookup binary-searches line starts, selects the nearest preceding
checkpoint where needed, and decodes at most one stride plus a crossing scalar
or maximal invalid run.

The detailed representation, lifecycle, bounds, and oracle cases are recorded
in the [line-index specification](../specifications/sparse-line-index.md).

## Rationale

A full byte-to-position map consumes unnecessary memory, while repeated scans
make conversion cost depend on the amount of preceding content. Sparse
checkpoints retain the common ASCII fast path and bound lookup work on long
Unicode or minified lines.

## Consequences

- The line index is not persisted in SQLite; final evidence coordinates are.
- Construction performs at most one complete scan per file revision within an
  operation and never publishes a partial index.
- The stride and cache lifetime remain private tuning parameters and may change
  without altering public coordinates.
- CRLF, invalid UTF-8, end-exclusive ranges, and boundary rejection continue to
  follow ADR-006.

## Reopen triggers

Reopen the representation if measured index memory becomes material,
construction dominates extraction, lookup violates its structural bound, or
the ADR-006 oracle cannot be implemented without ambiguous boundaries.
