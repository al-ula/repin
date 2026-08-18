# Experiment Result: F4 — Cancellation and concurrency model

Status: recorded (Linux PoC; hybrid-benefit audit unresolved; overall inconclusive)

- Lifecycle stage: experimentation
- Experiment specification: ../rust-foundation.md#5-f4-cancellation-and-concurrency-model
- Run ID: foundation-f4-confirm-20260818
- Platform scope: Linux x86_64/glibc only; Tier 2 and non-Linux expansion deferred

## Result

The confirmatory pass ran three serial full-profile replicates of the
disposable repin-f4-spike binary, pinned with taskset --cpu-list 0-3. The same
release binary was used for every run. Each replicate retained 72 cases and 54
measurements across the synchronous, hybrid adapter-boundary, and
Tokio-orchestrated models. All three models passed all 24 behavioral cases in
all three replicates; no hard blocker or case gap was observed.

The strict confirmatory gate failed on the hybrid performance requirement:
hybrid did not produce at least 25% p95 throughput improvement for either
service or remote workloads in any replicate. The derived benefit series were
also unstable under the gate because the measured benefits were near zero. The
result is therefore inconclusive, with the synchronous core retained as the
conservative default. A globally async core remains disallowed.

The retained evidence is in
[f4-confirmatory-20260818](raw/f4-confirmatory-20260818/). It contains each
replicate manifest, host metadata, F4.json, and F4-report.json, plus the
deterministic [aggregate.json](raw/f4-confirmatory-20260818/aggregate.json).
The existing [f4-tier1-20260818](raw/f4-tier1-20260818/) artifacts were left
unchanged.

## Hybrid-benefit audit

The controlled audit ran all 12 Linux loopback cells serially: pinned CPUs
0-3 and unpinned affinity, native and matched four-client accounting, and the
three fixed model-order rotations. Every cell ran two warmups and ten measured
samples for 64 service requests and 32 remote requests per model. All 12 cells
recorded zero errors, exact request completions, four server workers, and
bounded client/server activity.

The audit did not reproduce the original Tier-1 sync slowdown. The original
Tier-1 sync p95 was 994.264 requests/s for service and 382.744 requests/s for
remote; the confirmatory medians were 1,777.900 and 480.884 requests/s, a
78.816% and 25.641% increase. In the controlled probes:

| Controlled factor | Service sync p95 effect | Remote sync p95 effect | Gate result |
|---|---:|---:|---|
| Pinned minus unpinned, native, across rotations | -1.050% to -1.977% | -0.843% to -1.478% | not explained |
| Order rotation range, pinned/native | 0.763% | 0.271% | not explained |
| Native minus matched hybrid-vs-sync benefit | +1.866 to +4.935 percentage points | +0.388 to +2.056 percentage points | not explained |

The discrepancy is therefore classified as **unresolved**: neither affinity,
order/warmup, nor client-concurrency fairness meets the required 25% effect in
both workloads and two rotations with stable repeated samples. The original
run has no host metadata, so its environment cannot be reconstructed. Raw
probe samples, host sidecars, normalized diagnostic outputs, preserved-input
hashes, and derived evidence are retained in
[f4-hybrid-audit-20260818](raw/f4-hybrid-audit-20260818/), with the aggregate
in [aggregate.json](raw/f4-hybrid-audit-20260818/aggregate.json).

### Validation and behavioral evidence

| Replicate | Sync | Hybrid | Async | Normalized outcomes | F4 JSON/report bytes equal |
|---|---:|---:|---:|---|---|
| 01 | 24/24 pass | 24/24 pass | 24/24 pass | yes | yes |
| 02 | 24/24 pass | 24/24 pass | 24/24 pass | yes | yes |
| 03 | 24/24 pass | 24/24 pass | 24/24 pass | yes | yes |

The aggregation helper validates the exact 24 case IDs per model, 72 cases,
54 measurements, seven cancellation measurements with 30 samples each, and
ten throughput measurements with five samples each per model. Normalized
outcome comparison excludes timing and service/remote throughput fields while
retaining case IDs, expected outcomes, pass/fail outcomes, and invariant
details.

Across the retained runs, the highest cancellation sample was 2.431 ms,
below the documented 25 ms target. Deadline precedence, cancellation before
commit, cancellation during the atomic commit, and cancellation during derived
reconciliation passed for every model and replicate.

| Invariant | Confirmatory observation |
|---|---:|
| Queue capacity / maximum queue | 32 / 32 |
| Active workers / configured workers | 4 / 4 |
| Overflow roots escalated to rescan | 125 |
| Watch-coordinator cycles | 100 per model and replicate |
| Maximum watch shutdown | 17.633 µs |
| Maximum isolated-worker termination | 20.779 ms |
| Parser state or fact batch returned by isolated worker | neither |

### Throughput evidence

Values are requests/second. The table reports the cross-replicate median of
each statistic; the raw five-sample arrays for every model/workload are kept in
aggregate.json.

| Model | Service p50 / p95 / max | Remote p50 / p95 / max | p95 cross-run range (service / remote) |
|---|---:|---:|---:|
| sync | 1,764.7 / 1,781.6 / 1,781.6 | 477.7 / 480.3 / 480.3 | 0.644% / 0.681% |
| hybrid | 1,732.5 / 1,746.6 / 1,746.6 | 476.7 / 478.6 / 478.6 | 1.563% / 0.548% |
| async | 1,751.7 / 1,762.8 / 1,762.8 | 480.0 / 481.8 / 481.8 | 0.687% / 0.477% |

Hybrid p95 benefit is calculated as (hybrid - sync) / sync for the matching
replicate.

| Workload | Replicate 01 | Replicate 02 | Replicate 03 | Median | Benefit range | At least 25% in every run |
|---|---:|---:|---:|---:|---:|---|
| service | -2.057% | -0.836% | -2.992% | -2.057% | 104.783% | no |
| remote | +0.262% | -0.304% | -0.966% | -0.304% | 403.644% | no |

All service/remote p95 series, including async, stayed within a maximum
1.563% cross-run range. The derived hybrid-benefit series did not stay within
the 10% range because the benefits were close to zero; this is an evidence gap,
not a behavioral blocker.

### Host, build, and reproducibility evidence

- Target: Linux x86_64/glibc; process affinity was [0, 1, 2, 3] and online
  CPUs were 0-15 for every replicate.
- Host: AMD Ryzen 7 5825U with Radeon Graphics; kernel
  7.1.4-204.fc44.x86_64; CPU governor performance; taskset from util-linux
  2.42.2.
- Toolchain: rustc 1.97.1 (8bab26f4f 2026-07-14) and cargo 1.97.1
  (c980f486 2026-06-30).
- Recorded one-minute load averages were 2.3984, 1.9121, and 1.6128 for
  replicates 01-03. Load was recorded but was not an admission gate.
- The locked offline release build took 22,778 ms and produced a 5,739,880
  byte binary. The same value is recorded in every manifest through
  REPINF4_CLEAN_BUILD_MS.
- Fixture seed: repin-f4-1; each run used one warmup, 30 cancellation samples,
  and five throughput samples.
- Tokio remains optional at 1.53.1 with only rt-multi-thread, sync, time, net,
  io-util, and macros; the synchronous build remains free of Tokio.

## Recommendation

The strict confirmatory gate failed, and the controlled hybrid-benefit audit
left the reversal unresolved. Retain the synchronous core contract as the
conservative default and close runtime selection as inconclusive. Do not carry
forward the initial single-run hybrid provisional recommendation, and do not
adopt a globally async core: the synchronous model passed every mandatory
behavioral case and no confirmatory async-only advantage was demonstrated.

Any new full F4 selection run requires a separately approved plan. The audit
itself makes no runtime, public API, production dependency, architecture,
Tier 2, or non-Linux decision.

The overall ledger remains Linux PoC recorded/inconclusive. Tier 2, lower-tier,
non-Linux, and other platform work is not implemented and should begin only
after a separately approved post-PoC expansion plan.

## Limitations and follow-up

- This is Linux x86_64/glibc PoC evidence only. No macOS, Windows, or other
  platform artifact was produced.
- The in-memory mock store proves F4 atomicity semantics only; it is not evidence
  for the redb candidate.
- Loopback service and remote workloads exercise bounded protocol/concurrency
  behavior, not a production network or provider implementation.
- No public API, production dependency decision, or architecture document was
  changed by this confirmatory run.
