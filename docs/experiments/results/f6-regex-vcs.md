# Experiment Result: F6 — Direct regex and VCS adapters

```text
Status: complete (Tier 1 evidence retained; overall inconclusive)
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#7-f6-direct-regex-and-vcs-adapters
Run ID: foundation-tier1-20260818
Overall outcome: inconclusive
```

## Result

Both `regex` 1.13.1 and `regex-automata` 0.4.16 accepted the bounded literal,
Unicode, multiline, and alternation cases, returned identical byte spans for
the fixture, and rejected look-around and backreference syntax. The chunked
scan probe exposed a 64 KiB safe-point boundary.

The sanitized Git subprocess returned normalized changed paths and exposed a
missing-executable fallback. The feature-enabled `gix` 0.86.0 probe discovered
the same temporary repository, but did not yet compare its changed set against
the subprocess oracle. No hard blocker was observed; the missing workload
coverage is an evidence gap rather than a normative-contract failure.

## Provisional recommendation (decision deferred)

Retain both regex candidates and both VCS approaches as provisional inputs to
the next comparison pass. The initial observations support the bounded
contract and the subprocess security shape; they do not select an adapter or
establish compile, memory, or cancellation limits.

This recommendation is recorded for later plan finalization. The experiment
does not accept or reject a candidate or select an implementation default.

## Required follow-up

- add expensive-pattern compile/memory cases and real cancellation checks;
- compare `gix` and subprocess changed sets across branch switches, shallow
  history, submodules, worktrees, and rewritten revisions; and
- defer additional-platform normalization/fallback validation until the fully
  featured Linux PoC is complete.

## Evidence

- [feature-run batch report](raw/foundation-tier1-features-20260818/batch.json)
- [feature-run F6 report](raw/foundation-tier1-features-20260818/F6-report.json)
- [spike workspace](../foundation_spike/)
