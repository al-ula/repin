# ADR-007: Sequence optional capabilities after deterministic foundations

```text
Status: accepted sequencing decision
Date: 2026-08-19
Decision type: milestone scope
Supersedes: none
```

## Decision

Watching is deferred until I3, after the deterministic update protocol and
convergence behavior are implemented. Vector search is deferred until the
semantic-retrieval milestone (I5), where it begins with the exact Rust baseline
selected by ADR-012. An ANN alternative is evaluated only if the exact baseline
hits an ADR-012 reopen trigger.

No deterministic capability, benchmark claim, or release artifact may depend on
a vector adapter before that milestone. Absence of a vector adapter must leave
deterministic revisions and retrieval available.

## Evidence

F5 is intentionally deferred until I3. S3 is intentionally deferred, and the
prior review recorded no S3 run. The architecture already defines vector
retrieval as optional and requires degradation to semantic recall only.

## Consequences

- Storage/recovery work can be completed with the vector channel absent.
- Watcher events remain untrusted hints; startup scans and reconciliation stay
  authoritative when watching is implemented.
- Exact Rust vector search is selected for I5 by ADR-012, but remains absent
  from deterministic milestones and initial release claims.
