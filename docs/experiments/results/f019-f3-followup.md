# Experiment Result: F-019 — hash/read and update-protocol follow-up

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#4-f3-hashing-and-update-protocol
Result revision: working-tree-status: ae8b504320c2e1960da7b0ea46899aa406e2d75fe6e306e70fb68873224bbb44
Run ID: foundation-followup-20260818
Overall outcome: pass
```

## Question and method

Measure resident hash, file-read, and file-read-plus-BLAKE3 distributions over
all defined corpus bands, then exercise stale-plan reprepare/conflict behavior
and graph reconciliation against a fresh graph oracle.

The runner used deterministic bytes from `repin-foundation-followup-1` and
three samples per mode at 0, 1 KiB, 4 KiB, 64 KiB, 1 MiB, 16 MiB, 32 MiB, and
64 MiB. Filesystem cache eviction was not forced and is recorded as a
limitation.

## Results

| Requirement | Evidence | Outcome |
|---|---|---|
| Resident, file-read, and read-plus-BLAKE3 distributions | 24 measurements in `F019` report, three samples each | pass |
| Two automatic reprepare attempts then conflict with no stale commit | `F019-REPREPARE` and retained state JSON | pass |
| Create/delete/recreate/rename and duplicate resubmission | `F019-SEQUENCE`, `F019-COALESCING` | pass |
| Incremental graph equals fresh graph, including node identity | graph equality fields in report/artifact | pass |
| Same bytes from host/watcher/scan/VCS deduplicate to one tagged hash | `F019-ORIGIN-DEDUP` | pass |

The normalized state, sequence, and origin artifacts were byte-identical on a
repeat run. Timing samples are intentionally not byte-identical and remain raw
distribution evidence.

## Retained evidence

- [JSON report](raw/foundation-followup-20260818-v7/f019-report.json)
- [state, sequence, and origin-hash artifact](raw/foundation-followup-20260818-v7/artifacts/f019/state-and-sequences.json)

## Limitations and recommendation

The timings are Linux x86_64/glibc observations with the host filesystem cache
left in place; they are not admission thresholds. The retry and hashing rules
remain protocol evidence, not a production default. Recommended disposition:
`defer` final representation and threshold decisions.
