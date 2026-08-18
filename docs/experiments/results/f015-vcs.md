# Experiment Result: F-015 — VCS adapter comparison

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#7-f6-direct-regex-and-vcs-adapters
Result revision: working-tree-status: ae8b504320c2e1960da7b0ea46899aa406e2d75fe6e306e70fb68873224bbb44
Run ID: foundation-followup-20260818
Overall outcome: pass
```

## Question and method

Compare pinned `gix` 0.86.0 with a bounded Git subprocess adapter against one
normalized changed-path oracle. The subprocess uses an explicit executable,
cleared environment with only required variables restored, no shell, bounded
stdout/stderr, disabled hooks/config/aliases, and kill-and-reap cancellation.

## Results

| Matrix case | Outcome |
|---|---|
| Dirty tree and ignored-file omission | pass |
| Branch switch | pass |
| Linked worktree | pass |
| Shallow clone | pass |
| Submodule | pass |
| Rewritten history | pass |
| Missing Git executable | pass; full-scan fallback observable |
| Incompatible executable | pass; full-scan fallback observable |
| Cancellation and subprocess policy | pass |

For every repository state in the matrix, `gix` and subprocess changed paths
matched. Dynamic commit IDs are retained as metadata; repeat comparison strips
those IDs and produced a byte-identical normalized artifact.

## Retained evidence

- [JSON report](raw/foundation-followup-20260818-v7/f015-report.json)
- [VCS comparison artifact](raw/foundation-followup-20260818-v7/artifacts/f015/vcs-comparison.json)

## Limitations and recommendation

This is a disposable Linux x86_64/glibc comparison and does not select a
production adapter. Repository configuration and trust-policy expansion remain
separate follow-up work. Recommended disposition: `defer` selection while
retaining both approaches and the bounded fallback protocol.
