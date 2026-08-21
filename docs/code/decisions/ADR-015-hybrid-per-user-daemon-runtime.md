# ADR-015: Use a per-user daemon with an in-process engine surface

```text
Status: accepted implementation architecture for the initial Linux PoC
Date: 2026-08-19
Decision type: runtime topology and composition
Builds on: ADR-002, ADR-003
```

## Decision

Repin's normal local runtime is one on-demand daemon per unprivileged OS user.
Clients rendezvous with it through a private pathname Unix-domain socket. The
daemon hosts isolated per-project contexts keyed by canonical
`.repin/graph.sqlite3` paths, owns the user-wide singleton lease, and holds each
active project's writer lock.

The engine also retains `open(EngineOptions) -> Engine` as an in-process
composition surface for deterministic tests, daemon construction, and library
embedding where the host explicitly owns the lifecycle. It is not a second
normal client topology and does not let ordinary clients bypass daemon-owned
project coordination.

Implementation proceeds engine first: establish the deterministic core and
port conformance in process, then wrap it with the daemon, project registry,
and bounded protocol. The daemon path must be complete before multi-client
sharing, watcher ownership, or the normal project-bound CLI is considered
conforming.

The detailed rendezvous, locking, discovery, protocol, and lifecycle contract
is normative in [Runtime and IPC](../runtime.md).

## Rationale

Compared with an in-process-only runtime, the per-user daemon provides one
writer authority, shared warm project contexts, centralized watcher ownership,
and coherent revisions across multiple clients. Compared with one daemon per
project, it provides a single stable rendezvous point and avoids a process and
socket for every active project.

Retaining the in-process engine surface keeps deterministic core tests and
explicit library embedding simple without turning that surface into a competing
deployment model. Building the engine before the transport also keeps IPC and
lifecycle failures out of core conformance tests.

## Consequences

- IPC framing, daemon election, stale-socket recovery, bounded startup, and
  context lifecycle are accepted implementation costs.
- Project stores, locks, watchers, revisions, and derived work remain isolated
  by context, but a daemon-process crash temporarily interrupts every context
  hosted by that daemon. Each project recovers independently on restart.
- A client process may attach to multiple projects only through separate bound
  connections; there is no cross-project domain request surface.
- Exact socket framing, queue sizes, worker counts, and idle limits remain
  private where the runtime contract does not fix them.
- Remote and federated transports remain deferred and cannot weaken the local
  project-bound protocol or safety model.

## Required implementation validation

1. Concurrent cold starts elect one daemon and losing candidates reconnect
   without unlinking a live socket.
2. Crashes release the singleton and project leases; restart repairs stale
   rendezvous state and recovers projects independently.
3. Protocol mismatch, malformed or oversized frames, bounded admission,
   deadlines, cancellation, and client disconnects fail without affecting
   unrelated requests.
4. Canonical-path context sharing, physical-alias rejection, writer-lock
   contention, and observer mode satisfy the runtime contract.
5. Context eviction and final daemon shutdown respect active clients, commits,
   recovery work, and watcher ownership.
6. Failure or cancellation in one project context cannot corrupt or mutate
   another project's state.

These checks validate conformance; they do not leave the topology decision
open by default.

## Reopen triggers

Reopen the topology if the shared process blast radius is unacceptable, the
bounded IPC and lifecycle implementation blocks the PoC, normal target hosts
require a daemon-free public topology, or measurements show that per-project
process isolation is worth its rendezvous and resource cost.
