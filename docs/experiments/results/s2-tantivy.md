# Experiment Result: S2 — Tantivy lexical adapter

```text
Status: pending
Lifecycle stage: experimentation
Experiment specification: ../storage.md#4-experiment-s2-tantivy-lexical-adapter
Overall outcome: not run
```

## Result

No S2 run has been retained. The specification defines the document schema,
exact and code-split fields, offset regions, lexical evidence fixtures, stale
verification, repair alternatives, and failure behavior against the
authoritative working tree.

## Recommendation

Keep Tantivy 0.26.1 as an evidence pin only. The C-005 re-read/hash/range
verification contract is normative; a lexical adapter cannot bypass it merely
because an index-derived range is faster.

## Required evidence

- exact term and code-split queries return normalized, bounded evidence;
- stale, deleted, and revision-mismatched documents are suppressed or repaired;
- index-derived ranges agree with re-read ranges before public exposure;
- repair and full rebuild converge with honest pending/lag status; and
- mmap/native behavior fits the support and failure-injection matrix.
