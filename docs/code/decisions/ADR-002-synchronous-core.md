# ADR-002: Use a synchronous core with explicit cancellation

```text
Status: accepted PoC default; revisitable
Date: 2026-08-19
Decision type: runtime/concurrency policy
Supersedes: none
```

## Decision

The Linux PoC uses a synchronous core. Operations expose explicit cancellation
and deadline checks, and bounded worker pools may be used for isolated work.
The core does not take a global async-runtime dependency as a baseline.

Adapter or service boundaries may use asynchronous orchestration when their
port contract requires it, but that does not make the domain core globally
async.

## Evidence

F4's three serial full-profile replicates passed all mandatory behavioral cases
for the synchronous, hybrid, and async models. The strict hybrid performance
gate failed, and the controlled audit did not demonstrate a confirmatory
async-only advantage. The prior review recommendation explicitly says not to
adopt a globally async core.

## Consequences

- Cancellation is a domain contract rather than a property delegated to a
  runtime scheduler.
- The PoC avoids carrying Tokio or another async runtime through every layer.
- Service adapters can still translate transport cancellation, deadlines, and
  disconnects into the same core signal.

## Revisit condition

Reopen this decision only with a separately approved experiment that specifies a
concrete workload, a measurable threshold, and evidence that a different core
model improves behavior without weakening cancellation or isolation.
