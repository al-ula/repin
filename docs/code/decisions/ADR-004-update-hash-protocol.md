# ADR-004: Preserve tagged content identity and stale-snapshot semantics

```text
Status: accepted contract decision; representation deferred
Date: 2026-08-19
Decision type: incremental update protocol
Supersedes: none
```

## Decision

Content identity is represented as an algorithm-tagged hash. BLAKE3 is the
current PoC candidate for the hash algorithm, but the public and persisted
shape always carries the algorithm identifier with the raw digest.

The update protocol must:

1. read and prepare from a snapshot;
2. detect mutation before commit;
3. automatically reprepare at most twice;
4. return a conflict without committing stale facts if the file still changes;
5. deduplicate identical bytes reported by host, watcher, scan, or VCS origins;
6. converge to the same graph as a fresh rebuild, including node identity.

## Evidence

F-019 passed the read/hash distributions, two-retry conflict case, create/delete/
recreate/rename sequence, duplicate resubmission, cross-origin deduplication,
and incremental-versus-fresh graph equality cases.

## Consequences

- Hashes are useful for content identity, cache keys, and update coalescing but
  never participate in entity identity.
- Stale evidence cannot become an acknowledged graph revision.
- Hash thresholds, storage representation, and cache policy remain
  implementation choices until plan finalization.

## Not decided

This ADR does not accept redb or any other store, and it does not set an
admission-latency or file-size threshold from the Linux measurements.
