# Experiment Result: F-009 — content-sniffing comparison

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#3-f2-filesystem-discovery-and-containment
Result revision: working-tree-status: ae8b504320c2e1960da7b0ea46899aa406e2d75fe6e306e70fb68873224bbb44
Run ID: foundation-followup-20260818
Overall outcome: pass
```

## Question and method

Compare the bounded in-house 8 KiB classifier with the pinned `infer` 0.19.0
comparator using the F-018 labeled corpus. Binary is the positive class;
unknown maintained-crate results are not silently treated as text.

## Results

The corpus contains 11 fixed-seed rows with retained BLAKE3 digests.

| Candidate | TP | FP | FN | TN | Unknown | Precision | Recall |
|---|---:|---:|---:|---:|---:|---:|---:|
| In-house prefix classifier | 6 | 0 | 0 | 5 | 0 | 1.00 | 1.00 |
| `infer` 0.19.0 | 2 | 0 | 0 | 0 | 9 | 1.00* | 1.00* |

`*` The maintained-crate metrics are only over its two classified rows; the
nine unknowns are an explicit coverage limitation, not evidence of text.
The provisional policy is: explicit exclusions win, inspect at most 8 KiB,
and return binary-or-diagnostic/skip when the bounded check cannot establish
safe text classification. No false-positive error or fail-open behavior was
observed in this corpus.

## Retained evidence

- [JSON report](raw/foundation-followup-20260818-v7/f009-report.json)
- [sniff decision and confusion metrics](raw/foundation-followup-20260818-v7/artifacts/f009/sniff-decision.json)
- [F-018 labeled source corpus](raw/foundation-followup-20260818-v7/artifacts/f018/sniff-corpus.json)

## Limitations and recommendation

The corpus is a deterministic Linux PoC fixture, not a production prevalence
sample. `infer` classified only two rows, so the comparison does not justify a
dependency decision. Recommended disposition: `defer` selection and retain the
in-house check provisionally with fail-closed diagnostics.
