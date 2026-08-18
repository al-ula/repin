# Experiment Result: F-020 — F6 aggregate follow-up

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#7-f6-direct-regex-and-vcs-adapters
Result revision: working-tree-status: ae8b504320c2e1960da7b0ea46899aa406e2d75fe6e306e70fb68873224bbb44
Run ID: foundation-followup-20260818
Overall outcome: pass
```

## Question and method

Aggregate the completed F-014 regex bounds/cancellation evidence and F-015
complete VCS comparison matrix into the F6 evidence set without selecting a
production adapter.

F-020 ran last in the dependency order and required the retained
`f014-report.json` and `f015-report.json` files. It records 12 F-014 cases and
11 F-015 cases, plus an explicit no-selection case.

## Results

| Requirement | Evidence | Outcome |
|---|---|---|
| Regex syntax, expensive-pattern bounds, exact spans, and cancellation retained | `F020-F014-INPUT` | pass |
| Complete gix/subprocess VCS matrix retained | `F020-F015-INPUT` | pass |
| No production adapter or default selected | `F020-NO-SELECTION` | pass |

The normalized F6 inputs were byte-identical on the repeat run. The aggregate
is evidence only; its decision status remains deferred.

## Retained evidence

- [JSON report](raw/foundation-followup-20260818-v7/f020-report.json)
- [F6 aggregate artifact](raw/foundation-followup-20260818-v7/artifacts/f020/f6-aggregate.json)
- [F-014 result](f014-regex.md)
- [F-015 result](f015-vcs.md)

## Limitations and recommendation

No production dependency, adapter, or default is selected. The evidence is
Linux x86_64/glibc only and remains subject to plan-finalization review.
Recommended disposition: `defer`.
