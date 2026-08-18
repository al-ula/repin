# Experiment Result: F-018 — F2 adversarial filesystem follow-up

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#3-f2-filesystem-discovery-and-containment
Result revision: working-tree-status: ae8b504320c2e1960da7b0ea46899aa406e2d75fe6e306e70fb68873224bbb44
Run ID: foundation-followup-20260818
Overall outcome: pass
```

## Question and method

Does the Linux capability-relative discovery/open protocol remain bounded and
fail closed across the complete adversarial path matrix, while reconciliation
and labeled sniff evidence remain separate from policy selection?

The exact command, seed, pins, and environment are in the [run manifest](raw/foundation-followup-20260818-v7/manifest.json).
The run order was `F-017 -> F-018 -> F-009 -> F-019 -> F-014 -> F-015 -> F-020`.

## Results

| Matrix area | Cases/evidence | Outcome |
|---|---|---|
| Selection precedence and exclusions | `F018-SELECTION`; selected paths retained | pass |
| Normal, traversal, absolute, and symlink escape paths | `F018-P-NORMAL`, `F018-P-TRAVERSAL`, `F018-P-ABSOLUTE`, `F018-P-ESCAPE` | pass |
| Cycles and depth limit | `F018-P-CYCLE`, `F018-P-DEEP` (depth 40, limit 32) | pass |
| Non-UTF-8 and case behavior | `F018-P-ENCODING`, `F018-P-CASE` | pass |
| Component swaps and concurrent mutation/reconciliation | `F018-P-SWAP` (32 attempts), `F018-P-MUTATE` | pass |
| Content-sniff corpus retained without selecting F-009 policy | `sniff-corpus.json`, `F018-SNIFF-*` | pass |

Every omitted path has a reason in the path manifest: ignore/glob exclusion,
pre-open traversal/absolute rejection, no-follow escape/cycle handling, depth
limit, or post-read containment/identity validation. The repeated normalized
path manifest and sniff corpus were byte-identical.

## Retained evidence

- [JSON report](raw/foundation-followup-20260818-v7/f018-report.json)
- [path manifest and omission reasons](raw/foundation-followup-20260818-v7/artifacts/f018/path-manifest.json)
- [labeled sniff corpus](raw/foundation-followup-20260818-v7/artifacts/f018/sniff-corpus.json)
- [F-008 Linux race report](f008-root-capability.md)

## Limitations and recommendation

The run qualifies only Linux x86_64/glibc. It does not establish behavior on
case-insensitive filesystems or other platform path APIs. The sniff corpus is
an input to F-009; this report deliberately makes no content-policy choice.

Recommended disposition: `defer` the production filesystem and sniffing
decisions while retaining the capability-relative protocol as a provisional
candidate.
