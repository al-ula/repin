# Host Integration

How an application embeds Repin. This is L4 in [Architecture](architecture.md).

The design goal is that an adapter is **thin and unprivileged**: it translates vocabulary and renders results, and does nothing that requires reaching past the public API. If an adapter needs internal access to work well, that is a defect in this seam, not in the adapter.

## 1. Adapter responsibilities

An adapter owns:

- translating host request vocabulary into engine calls
- translating engine results into host output format
- host-side lifecycle binding (daemon connection, detachment, cancellation)
- host-side permission and trust policy
- reporting engine state in whatever way the host expresses state

An adapter does **not** own:

- normalizing results (L3 does this)
- ranking or fusion (L2)
- budgeting or truncation (L3)
- path validation or redaction (L1–L3, enforced engine-side)
- deciding what is fresh (the engine reports; the host displays)

The division matters because there will be several adapters. Anything an adapter is allowed to reimplement, it will reimplement differently, and then two clients will disagree about the same repository.

## 2. Capability negotiation

An adapter MUST ask what exists rather than assume a fixed feature set.

```text
capabilities() -> Capabilities

Capabilities
  retrieval:  Mode[]              // files, text, regex, symbol, related
  graph:      GraphCapabilities
  relations:  RelationKind[]
  entities:   EntityKind[]
  artifactClasses: ArtifactClass[]
  semantic:   bool
  rerank:     bool
  context:    ContextStrategy[]
  limits:     Limits
  providers:  ProviderInfo[]

ProviderInfo
  id:        ProviderId
  kind:      parser | resolver | store | lexical | vector | model | host | other
  version?:  Version
  location:  local | remote | host_supplied

GraphCapabilities
  entity:        bool
  neighbors:     bool
  trace:         bool
  impact:        bool
  relatedSearch: bool
```

Provider IDs are opaque public aliases registered by the composition root.
They need not expose the internal crate, executable, endpoint, or extractor
identity. Every provider that contributes public evidence must be registered;
the engine maps private producer claims to that alias before returning results.
Versions are separate metadata and provider responses cannot invent or replace
the registered identity. See [Results and Evidence — Public provider identity](results.md#public-provider-identity).

Rules:

1. Negotiate once per session, refresh on explicit invalidation. Do not negotiate per request.
2. Offer only what is present. An operation the engine cannot perform should not be advertised to the host's users, and MUST NOT be advertised to an automated caller — an agent shown a capability will attempt it.
3. Unknown values in these lists are tolerated and ignored, never fatal. This is what lets the engine add a relation kind without breaking existing adapters.
4. Requesting an unsupported filter returns `CAPABILITY_UNSUPPORTED` **with the supported set**, so a caller can correct itself in one round trip instead of guessing.

## 3. Lifecycle

Four phases, with a strict rule about the first one.

**Construct.** Cheap, synchronous, side-effect free. No store handles, no
threads, no watchers, no project locks, no network connection, and no
filesystem scan. A host that constructs an adapter and never uses it pays
nothing. This is not a micro-optimization: hosts commonly load every
available integration at startup, and an integration that opens a database on
construction makes itself unloadable.

**Activate.** Explicit. Connect to the per-user daemon and select a project
with `DiscoverFrom` or `AtRoot`; if the socket is absent, the client starts a
bounded detached daemon-candidate handshake. The daemon resolves roots, loads
configuration, opens the context, and performs a bounded health check. An
invalid or newer graph may produce a degraded client with direct retrieval
still available. Activation may run lazily on first use, provided the first
use pays a bounded cost and reports `INDEX_BUILDING` or the precise runtime
error rather than blocking indefinitely.

**Serve.** Each operation takes a cancellation signal and a deadline. Long
operations may emit the API progress events defined in [Public API — Progress
events](api.md#progress-events). A host may coalesce or render those events,
but it does not redefine their stage or completion semantics.

**Detach.** `ProjectClient.close()` cancels or completes only work owned by
that connection according to the protocol, closes the connection, and clears
host-visible state. It does not release another client's context, terminate
the daemon, or cancel unrelated requests. The daemon decides when the context
is idle; after `600,000 ms` without clients, in-flight work, authoritative
commits, or mandatory recovery, it stops the watcher, closes stores and
indexes, and releases the project writer lock.

**Shutdown.** The daemon stops only after the final context unloads and no
bootstrap or client connection remains. It closes the central socket before
releasing the per-user daemon lease. Shutdown is idempotent and must complete
promptly; a shutdown that waits for a full index to finish is a hang.

## 4. Change notification

A host that edits files knows about the edit before the filesystem watcher does. Telling the engine directly is the difference between an edit being queryable in milliseconds and being queryable after a debounce interval plus an update cycle.

```text
notifyChanges(changes: FileChange[]) -> UpdateSummary
```

Rules:

- Notify **after** the write completes, not before.
- Include content when the host already has it in memory; the engine then skips a read.
- Mark the origin ([Incremental Updates](incremental.md)). Origin is used for deduplication and diagnostics only, never to alter extraction results.
- Do not suppress the watcher. Edits happen outside the host constantly — other tools, other processes, the user's editor.
- Deduplication is by content identity, so a host notification and the watcher event for the same write produce one update. A host must not implement its own timing-based suppression; it will be wrong under load.

For known bulk operations — a branch switch, a dependency install, a code generation run — a host SHOULD wrap the operation in the engine's pause/resume so thousands of events become one batch. This is an optimization; the engine must survive the unannounced case regardless.

## 5. Freshness surfacing

The engine reports; the host decides what to show.

- The engine MUST NOT block a query to become fresh. Answer now, label the state.
- The engine MUST NOT serve stale graph facts without saying so.
- The host SHOULD surface `building`, `stale`, and `unavailable` states, and SHOULD stay quiet when everything is healthy. Persistent healthy-state indicators become noise, and noise gets ignored, including when it stops being healthy.
- When graph facts and current file content conflict, the working tree wins ([Architecture §1](architecture.md#1-two-capabilities-one-product)) and the conflict is reported.

## 6. Guidance for automated callers

When the host is an agent, the adapter's description of each operation is part of the interface, and vague descriptions produce misuse. The engine does not own the host's prompt vocabulary, but it can state what each operation is *for*:

- entity resolution — inspect or disambiguate one known entity
- neighbors — immediate relationships around an entity
- trace — bounded "how does A reach B?"
- impact — evidence-backed candidates affected by changing something
- search — find evidence when the target is not yet known
- context — assemble material for a budgeted consumer

Two properties must be conveyed to any automated caller, because getting them wrong produces confidently wrong output:

1. **Impact results are candidates, not consequences.** They are bounded, evidence-backed, and never claimed exhaustive.
2. **Current file content outranks graph facts.** After reading a graph result, verifying against the working tree is correct behavior, not redundant.

## 7. Provider-contract hosts

Some hosts already define their own retrieval vocabulary and want the engine behind it as one interchangeable provider among several. This is a supported and preferred pattern, and it is what keeps the engine from accumulating host-shaped API.

```text
Provider
  health()        -> Health
  capabilities()  -> Capabilities
  status()        -> IndexStatus
  resolveEntity(request) -> Result<Resolution<Entity>>
  neighbors(request)     -> Result<Relationship[]>
  trace(request)         -> Result<Path[]>
  impact(request)        -> Result<ImpactGroup[]>
  searchRelated(request) -> Result<Entity[]>
  refresh(request)       -> Result<RefreshOutcome>
  close()
```

Requirements on the engine as a provider:

- accept cancellation and a deadline on every operation
- enforce its own timeouts; never rely on the host to bound it
- return machine-readable errors from the taxonomy in [Results and Evidence](results.md)
- declare unsupported operations explicitly rather than failing obscurely
- return freshness and coverage on every graph answer
- never require the host to understand storage, indexing, or identity internals

Two consequences worth stating plainly:

**The host owns its tool surface; the engine does not compete with it.** If a host defines its own operation names, the engine implements that contract and registers nothing of its own. Two overlapping surfaces for one capability is a defect, and the host's surface wins because it is the one users see.

**Engine-native concepts that a host contract lacks** — revisions, change history, context strategies — are exposed through `status()`, `refresh()`, and richer non-host clients (CLI, local daemon protocol). They do not motivate adding host-facing operations. The CLI and daemon protocol are not bound by a host's prompt-cost or vocabulary constraints, so that is where full expressiveness lives. A future remote transport may reuse the protocol but is not part of the initial runtime.

## 8. Configuration

Configuration values merge from lowest to highest precedence:

```text
engine conservative defaults
  < user configuration
  < trusted-project configuration
  < explicit CLI/API overrides
```

Rules:

- The engine works with no configuration file.
- Project configuration is read only when the host indicates the project is trusted. If trust is false or indeterminate, the project layer is ignored and a diagnostic states why; user configuration still applies.
- Environment variables are not a generic override layer. Configuration may contain typed references to credentials or deployment values supplied by the environment or a host credential store. Referenced values are resolved only by the composition root, are never written back or displayed, and a missing reference is reported as a capability configuration error.
- Configuration contains no literal secrets. CLI arguments should not carry secrets because process listings and shell history may expose them.
- Higher-precedence layers may narrow selection, reduce resource limits, disable capabilities, or choose among administrator-allowed adapters. No layer may weaken engine safety floors: root containment, default secret exclusions, no-execution, redaction, fail-closed behavior, mandatory resource maxima, and remote-provider trust/consent requirements remain enforced independently of merge precedence.
- Unknown fields and invalid types produce diagnostics. Configuration is
  schema-versioned; its migration is owned by the configuration loader, while
  graph-store schema migration is owned by the store adapter. See [Storage —
  Migration ownership](storage.md#migration-ownership).
- Capabilities are configured individually ([Optional Intelligence](intelligence.md)), never through one global on/off switch. A single flag cannot express "lexical yes, embeddings no, reranking via the host's model".

## 9. Multi-client behavior

One global daemon may host many isolated project contexts, and each context
may have many bound client connections. Exactly one authoritative writer exists
per project database, with concurrent readers where supported.

- Connections bound to the same canonical database path share one context,
  warm indexes, watcher, and committed revision. A second connection does not
  open a second store or compete for the project lock.
- The daemon, not a client, holds `.repin/writer.lock`. A client never claims
  or deletes it. If an external process owns the lock, the daemon may attach
  an observer context where safe; direct retrieval remains available and graph
  writes return `PROJECT_LEASE_UNAVAILABLE`.
- Read-only or observer clients observe durable graph progress by comparing
  revisions for equality, not by consuming a revision event stream. Individual
  operations may still receive the advisory API progress events defined in
  [Public API — Progress events](api.md#progress-events).
- A client asking for changes since a compacted revision receives
  `REVISION_TOO_OLD` and resyncs. It MUST NOT receive an incomplete delta.
- Closing or losing one client connection does not terminate the daemon or
  cancel unrelated project work.
