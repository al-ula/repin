# ADR-005: Keep direct search bounded and explicit

```text
Status: accepted contract decision; initial adapters selected by ADR-010 and ADR-011
Date: 2026-08-19
Decision type: direct retrieval
Supersedes: none
```

## Decision

Direct regex search advertises only measured syntax. Unsupported constructs
return an explicit invalid-query outcome rather than changing pattern meaning.
Matches carry exact spans, compile and expansion work is bounded, and
cancellation checkpoints are explicit.

VCS change detection uses one normalized result shape. A subprocess fallback,
where used, runs without a shell, with sanitized environment and bounded
stdout/stderr, prompt cancellation, and a full-scan fallback when Git is
missing or incompatible.

## Evidence

F-014 found equivalent behavior for the bounded syntax and explicit rejection
of look-around/backreferences, with cancellation at adapter checkpoints. F-015
found matching normalized changed sets across the gix and subprocess candidates
for the Linux matrix, including shallow clones, worktrees, submodules, rewritten
history, missing Git, and cancellation. F-020 reported the aggregate without
selecting an adapter.

## Consequences

- Tantivy's lexical regex behavior cannot silently define direct regex
  semantics.
- Results remain safe and explainable when a candidate cannot support a mode.
- [ADR-010](ADR-010-regex-direct-search.md) selects `regex` for the initial
  direct-search adapter.
- [ADR-011](ADR-011-bounded-git-subprocess.md) selects a bounded Git subprocess
  for the initial VCS adapter.
