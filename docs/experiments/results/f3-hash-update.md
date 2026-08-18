# Experiment Result: F3 — Hash and update preparation protocol

```text
Status: complete (Tier 1 evidence retained; overall inconclusive)
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#4-f3-hash-and-update-preparation-protocol
Run ID: foundation-tier1-20260818
Overall outcome: inconclusive
```

## Result

The spike demonstrated tagged BLAKE3 hashes, origin-independent deduplication,
stale snapshot rejection after file mutation, byte preservation across rename,
and stable identity independent of content hash. Resident hash measurements
were recorded at 0 B, 1 KiB, 4 KiB, 64 KiB, 1 MiB, and 4 MiB.

The run did not yet provide separate cold file-read versus read-plus-hash
distributions, a full two-reprepare conflict sequence, or the generated
create/delete/recreate and coalescing matrix. No hard blocker was observed;
these are evidence gaps rather than normative-contract failures.

## Provisional recommendation (decision deferred)

Retain BLAKE3 and the tagged `InputSnapshot` representation as provisional
baselines for follow-up experiments. The stale-plan boundary is supported by
the spike, but no production retry, hash, or performance decision is made.

This recommendation is recorded for later plan finalization. The experiment
does not accept or reject a candidate or select an implementation default.

## Required follow-up

- add file-read and read-plus-hash measurements for all corpus bands;
- exercise the two-reprepare budget and conflict outcome; and
- run generated rename/coalescing sequences against the graph-equality oracle.

## Evidence

- [feature-run batch report](raw/foundation-tier1-features-20260818/batch.json)
- [feature-run F3 report](raw/foundation-tier1-features-20260818/F3-report.json)
- [spike workspace](../foundation_spike/)
