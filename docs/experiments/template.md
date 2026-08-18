# Experiment Result: {ID} — {title}

Use one copy of this template per experiment run or comparable run group. Keep raw data outside this document when large, but link it with a stable repository-relative path and checksum. Do not replace an anomalous run; record and explain it.

```text
Status: planned | running | complete | invalidated
Lifecycle stage: experimentation
Experiment specification: {repository-relative link}
Result revision: {source revision}
Run ID: {stable identifier}
```

## 1. Question and hypothesis

**Question:** {the contract or candidate uncertainty being tested}

**Hypothesis:** {falsifiable expected result}

**Decision enabled:** {the plan-finalization decision this evidence informs}

A result records evidence and a recommendation. It does not accept a production dependency or silently turn spike code into production code.

## 2. Contract traceability

| Requirement or pass condition | Source | Case(s) | Result |
|---|---|---|---|
| {requirement} | `{document} §{section}` | {case IDs} | pending |

List every applicable pass condition. Record intentionally untested conditions as gaps rather than omitting them.

## 3. Candidate and implementation pins

| Component | Version/revision | Source/checksum | Features/build options |
|---|---|---|---|
| {crate/tool/grammar/query} | {exact pin} | {registry/repository/checksum} | {flags} |

- Spike source: `{path or revision}`
- Lockfile/checksum: `{path and digest}`
- Build profile and flags: `{debug/release and flags}`
- Rust toolchain: `{rustc and cargo versions}`

## 4. Environment

| Field | Value |
|---|---|
| Platform scope | Linux x86_64/glibc PoC / post-PoC platform expansion |
| OS and version | |
| Architecture | |
| CPU | |
| Memory | |
| Storage medium/device | |
| Filesystem and mount options | |
| Free space before run | |
| Host/VM/container | |
| Relevant limits/configuration | |

State whether the run is part of the complete Linux PoC qualification or a
later, explicitly identified platform-expansion run.

## 5. Fixtures and workload

| Fixture/corpus | Version/checksum | Composition | Expected oracle |
|---|---|---|---|
| | | | |

Record:

- random seeds and generation parameters
- file, byte, node, edge, and query counts as applicable
- graph shape, including fan-in/fan-out where relevant
- cold/warm state and how it was established
- concurrency, queue, batch, and resource bounds
- pre-run state layout and revision

## 6. Method

Number each case so it can be traced to a pass condition.

### Case {ID} — {name}

1. {setup}
2. {operation or fault injection}
3. {observation and oracle}
4. {cleanup/reopen/retry as applicable}

Commands must be directly reproducible:

```sh
{exact commands without unrecorded shell state}
```

Document instrumentation overhead and deviations from the experiment specification before presenting results.

## 7. Results

### Correctness and behavior

| Case | Expected | Observed | Pass/fail/gap | Evidence |
|---|---|---|---|---|
| | | | | `{path/link}` |

### Measurements

| Case/metric | Unit | Runs | Distribution/variance | Raw data |
|---|---|---:|---|---|
| | | | | `{path/link}` |

Report distributions and scaling curves when no approved threshold exists. Label extrapolation explicitly. Keep crawl, read, hash, detect, parse, extract, resolve, store, lexical, and vector costs separate where applicable.

## 8. Failure and recovery evidence

| Injection point or seed | Durable state before reopen | State after reopen/repair | Reproducer retained at |
|---|---|---|---|
| | | | `{path/link}` |

Include process exit mode, state-directory layout, authoritative and derived revisions, pending work, warnings, and whether retry/rebuild was required. Retain every minimized failure seed as a regression fixture.

## 9. Pass-condition evaluation

| Pass condition | Outcome | Evidence or unresolved gap |
|---|---|---|
| | pass / fail / inconclusive / not run | |

Overall experimental outcome: `pass | fail | inconclusive | invalid run`

An overall pass requires every mandatory condition to pass. Unsupported platform behavior or an omitted case is a gap, not a pass.

## 10. Limitations and threats to validity

- {fixture representativeness}
- {platform/configuration not exercised}
- {measurement noise or instrumentation effects}
- {behavior delegated to an unpinned external component}
- {difference between spike and prospective production adapter}

## 11. Evidence-based recommendation

Recommended disposition for plan finalization: `accept | reject | defer | revise experiment`

**Reasoning:** {what the retained evidence supports}

**Known costs and capability effects:** {limits, degradation, portability, security, operations}

**Required follow-up:** {new experiment/task/ADR input}

This recommendation remains provisional until plan finalization records the decision.

## 12. Artifact inventory

| Artifact | Path/link | Checksum or revision | Retention reason |
|---|---|---|---|
| Method/scripts | | | reproducibility |
| Raw measurements | | | independent analysis |
| Logs/traces | | | diagnosis |
| Fixtures/seeds | | | regression/reproduction |
| State snapshots | | | recovery evidence |
| Summary plots | | | review |
