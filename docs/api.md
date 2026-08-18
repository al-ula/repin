# Public API

The surface clients depend on. Everything not described here is internal and may change without notice.

Notation is the neutral form described in the README. Operations are shown synchronously; whether an implementation returns eagerly or as a deferred value is a host-language choice, not part of this contract.

## 1. Project-bound client surface

Normal callers connect to a project through the user-scoped daemon described in
[Runtime and IPC](runtime.md). A client selects or initializes one project
during its first handshake; all later domain requests are implicitly scoped to
that project and do not repeat a project path.

```text
ProjectSelector
  = DiscoverFrom { path: Path }
  | AtRoot      { root: Path }

initializeProject(spec: ProjectSpec, call?: CallOptions)
  -> Result<ProjectClient>

connectProject(selector: ProjectSelector, call?: CallOptions)
  -> Result<ProjectClient>
```

```text
ProjectSpec
  root:          Path                  // project root to initialize
  roots?:        RootSpec[]             // optional logical roots in the graph
  config?:       Configuration
  capabilities?: CapabilityConfig
```

`ProjectSpec` has no `ProjectId`. The initialized project is addressed by the
canonical path to `.repin/graph.redb`; `root` and any logical roots are
configuration, not a second runtime identity. `initializeProject` creates the
state directory and database only when no database already exists. It acquires
the project writer lease through the daemon and returns a bound client.

`connectProject` performs ancestor discovery or explicit-root validation and
returns a bound client. An incomplete marker returns
`PROJECT_NOT_INITIALIZED`; an invalid or newer existing database returns a
degraded client with explicit graph-unavailable status when safe, as specified
in [Runtime and IPC — Initialization and graph capability](runtime.md#4-initialization-and-graph-capability).

```text
ProjectClient
  capabilities() -> Capabilities
  status()       -> IndexStatus
  close()        -> void             // detach this connection only
```

The remaining operations in this document are methods on `ProjectClient`.
`close()` does not terminate the user-wide daemon, release unrelated contexts,
or cancel requests belonging to other connections. A client must establish a
new bound connection to work with another project.

### Daemon-internal engine construction

The following composition surface is retained for the daemon and deterministic
engine tests. It is not the normal client entrypoint and does not transfer
project-lock ownership to the caller.

```text
open(options: EngineOptions) -> Engine

EngineOptions
  roots:        RootSpec[]              // required, at least one
  ports:        PortBundle              // injected implementations
  config?:      Configuration
  capabilities?: CapabilityConfig
```

```text
RootSpec
  id:     RootId       // logical identity; stable across explicit relocation
  path:   Path         // current binding, canonicalized at activation
  label?: Text         // display only; never identity
```

`RootId` is caller/configuration-owned and unique within one engine. It is not
derived from the absolute path. Rebinding an existing ID to a new path is
allowed, emits a root-relocation diagnostic, and forces reconciliation of that
root before its state is reported current. Supplying a new ID for a moved path
starts a distinct logical root; the engine does not infer continuity from a
matching basename or filesystem contents.

```text
PortBundle
  filesystem:  Filesystem               // required
  store?:      Store                    // graph capabilities need it
  lexical?:    Lexical
  vector?:     Vector
  watch?:      Watch
  vcs?:        Vcs
  packs:       LanguagePack[]
  embedding?:  EmbeddingModel
  reranker?:   Reranker
  textModel?:  TextModel
  clock:       Clock
  logger?:     Logger
  metrics?:    Metrics
```

`open` is **cheap and side-effect free**: no store handle, no thread, no watcher, no network, no filesystem traversal. The daemon composes an engine/context explicitly; a test that opens an engine and never uses it pays nothing ([Host Integration §3](host-integration.md#3-lifecycle)).

Ports are injected here and nowhere else. This is the single point where implementation selection happens, and the reason nothing above L0 performs environment detection.

```text
activate(call?: CallOptions) -> Result<ActivationReport>
close()                      -> void
```

`activate` resolves roots, loads configuration, opens the store, and performs a bounded health check. It MAY be implicit on first use provided that first use is bounded and reports `INDEX_BUILDING` rather than blocking. Internal `close` cancels outstanding work, releases the context's handles, and is idempotent. A public `ProjectClient.close()` only detaches its connection; context unloading remains the daemon's responsibility.

## 2. Capabilities and status

```text
capabilities() -> Capabilities
status()       -> IndexStatus
health()       -> Health
```

Negotiate once per session; refresh on explicit invalidation. Never per request.

```text
IndexStatus
  state:             empty | building | current | stale | unknown
  revision?:         Revision
  writerMode:        writer | reader | observer
  coverage:          CoverageReport
  lexicalRevision?:  Revision
  lexicalPending?:   Count
  lexicalState?:     current | lagging | disabled | failing
  semanticRevision?: Revision
  semanticPending?:  Count
  semanticState?:    current | lagging | disabled | failing
  lastUpdate?:       UpdateSummary
  vcs?:              VcsState
```

```text
CoverageReport
  filesIndexed:      Count
  filesSkipped:      Count
  skipsByReason:     Map<SkipReason, Count>
  unresolvedRefs:    Count
  languagesIndexed:  LanguageId[]
  languagesUnsupported: LanguageId[]
```

`writerMode` is part of status rather than buried in a diagnostic because a client that believes it can write when it cannot will fail confusingly and late.

Graph, lexical, and semantic revisions are independent equality tokens. A missing revision means the index is disabled or has never completed; the corresponding state distinguishes those cases. A lagging lexical index is bypassed and repaired without rolling back the authoritative graph. Retrieval uses current graph/direct channels and, where the mode supports it, a current working-tree scan. Results warn about the bypass; coverage becomes partial only when the fallback cannot completely search the requested scope.

## 3. Updating

```text
update(request?: UpdateRequest, call?: CallOptions) -> Result<UpdateSummary>
updateFiles(changes: FileChange[], call?: CallOptions) -> Result<UpdateSummary>
rebuild(request: RebuildRequest, call?: CallOptions) -> Result<UpdateSummary>
pause() -> void
resume(call?: CallOptions) -> Result<UpdateSummary>

RebuildRequest
  target: graph | lexical | vector | all
```

- `update` — detect and apply changes since the last known state, using the VCS when available.
- `updateFiles` — the real primitive ([Incremental Updates §3](incremental.md#3-the-update-primitive)). Everything else feeds it.
- `rebuild(target: graph)` — discard and reconstruct authoritative graph state from selected working-tree content, then rebuild every enabled derived index from the new graph. The operation completes only when required lexical reconstruction is acknowledged; optional vector work may remain asynchronous and is reported in status.
- `rebuild(target: lexical)` — discard and reconstruct only the lexical index from the current authoritative graph. Graph revision/facts and vector state do not change.
- `rebuild(target: vector)` — discard and reconstruct only the enabled vector index from the current authoritative graph/content. Graph and lexical state do not change; unavailable/disabled vector capability is reported explicitly.
- `rebuild(target: all)` — explicit full state reconstruction; equivalent in dependency scope to `graph`, retained for operational clarity.
- `pause`/`resume` — bracket a known bulk operation.

A rebuild never advances or rewrites the authoritative graph revision merely for reconstructing a derived index. Destructive replacement occurs only after cancellation-safe preparation where possible. Authoritative commits and individual product commits are non-cancellable atomic sections; cancellation stops before the next such section, preserves the last valid state, and reports any pending derived work. Long-running phases may report the bounded, best-effort progress events defined in [Progress events](#progress-events).

All graph-mutating operations require authoritative writer mode. In reader or
observer mode they return `PROJECT_LEASE_UNAVAILABLE` with the current
`writerMode` rather than silently doing nothing. Direct working-tree retrieval
does not require graph writer ownership.

```text
revision()               -> Revision
changesSince(revision)   -> Result<ChangesResult>
```

`Revision` is **opaque**: comparable for equality only, never ordered or parsed.

## 4. Watching

```text
watch(request?: WatchRequest, call?: CallOptions) -> Result<WatchSession>

WatchSession
  onUpdate(handler: (UpdateSummary) -> void)
  onError(handler: (Error) -> void)
  stop()
```

Optional. Explicit notification and scanning are complete alternatives. A session is stopped by `stop` or by `close`, and `stop` is idempotent.

### Progress events

Long operations MAY emit best-effort progress through the call's optional
`ProgressSink`. Progress is an API contract, not a host-specific invention;
in-process callers receive the events directly and service/host adapters map
the same shape to their transport or UI. Progress is advisory and is never
needed to determine correctness, freshness, cancellation, or completion.

```text
ProgressSink
  onProgress(event: ProgressEvent) -> void

ProgressEvent
  operationId: OperationId
  stage:       ProgressStage
  state:       started | updated | completed | skipped | cancelled | failed
  completed?:  Count
  total?:      Count
  observedAt:  Timestamp
  message?:    Text              // redacted, human-readable only

ProgressStage
  activate | discover | read | hash | parse | extract | resolve
  commit | lexical | semantic | repair | watch | retrieve | rank
  context | finalize
```

Rules:

- `operationId` is opaque and valid only for the lifetime of the call. It is
  not a revision, a persistence key, or a permission token.
- A stage emits at most one `updated` event per 100 ms per operation, plus its
  first and terminal event. Stage transitions may be emitted immediately.
- `completed` is monotonic within a stage. `total` is omitted when unknown;
  clients MUST NOT manufacture percentages from an absent total.
- Event delivery is non-blocking and bounded. A slow, cancelled, or failed
  sink may lose progress events; the operation continues and the result does
  not become partial solely because progress was dropped.
- Cancellation still travels through `CallOptions.signal`; a progress sink
  cannot veto or extend a deadline. A cancelled operation returns `CANCELLED`
  and emits no false `completed` event for work that did not commit.
- Progress is not persisted in graph history or replayed after reconnect.
  Adapters that need durable state use revisions and status instead.

## 5. Retrieval

```text
search(request: SearchRequest, call?: CallOptions) -> Result<SearchHit[]>

SearchRequest
  query:         Text
  mode:          files | text | regex | symbol | concept
  filters?:      Filters
  expand?:       none | related
  contextLines?: Count
  limit?:        Count
  budget?:       OutputBudget
```

`limit` is a per-call bound, not an offset or continuation mechanism. v1
search and graph operations are deliberately non-pageable; a bounded response
reports truncation through the result envelope. A client that needs more must
narrow the request or issue a new call, which may observe a later revision.
Only `changesSince` uses an opaque revision token for synchronization.

```text
SearchHit
  entity?:     Entity          // when the hit resolves to a graph entity
  evidence:    Evidence[]
  score:       Score
  explanation: RankExplanation
  source:      Source
```

`mode: concept` and `expand: related` use graph or semantic channels when present and degrade gracefully to deterministic channels when absent—reporting the degradation as a warning, never hiding it or failing an otherwise useful search.

```text
entity(request, call?: CallOptions)    -> Result<Resolution<Entity>>
neighbors(request, call?: CallOptions) -> Result<Relationship[]>
trace(request, call?: CallOptions)     -> Result<GraphPath[]>
impact(request, call?: CallOptions)    -> Result<ImpactGroup[]>
```

```text
TraceRequest
  from:      EntityRef
  to:        EntityRef
  maxDepth:  Count            // required; conservative maximum enforced
  maxPaths?: Count
  relations?: EdgeKind[]

ImpactRequest
  target:    EntityRef
  maxDepth:  Count            // required
  filters?:  Filters
  limit?:    Count
```

`maxDepth` is required rather than defaulted on `trace` and `impact`. An unbounded traversal on a large graph is a denial-of-service against the caller, and a default that is safe for one repository is not safe for all of them. Forcing the choice makes the cost visible.

```text
EntityRef = ById { id: EntityId } | ByName { name: Text, kind?: NodeKind, pathHint?: Path }
```

## 6. Context

```text
context(request: ContextRequest, call?: CallOptions) -> Result<ContextBundle>

ContextRequest
  entities:  EntityRef[]
  strategy?: ContextStrategy[]
  budget:    OutputBudget
  include?:  { definitions?, uses?, docs?, tests?, config? }
```

```text
ContextBundle
  fragments: ContextFragment[]
  omitted:   OmissionReport
  budgetUsed: BudgetUsage

ContextFragment
  entity?:  Entity
  evidence: Evidence
  content:  Text
  reason:   Text              // why this was included
```

```text
OutputBudget
  maxBytes?:  Count
  maxLines?:  Count
  maxUnits?:  Count
  estimator?: BudgetEstimator // required when maxUnits is used

BudgetEstimator
  = callback((Text) -> Count)       // in-process binding
  | registered(EstimatorId, Version) // implementation available to the engine
```

The estimator exists because a budget expressed in model tokens cannot be computed without a specified encoding. Guessing an encoding is how a budget gets exceeded. In-process callers may supply a callback; service protocols use a negotiated, registered estimator available on the engine side. A transport that supports neither MUST reject `maxUnits` as unsupported and still accepts byte/line budgets—it MUST NOT silently substitute an approximate tokenizer.

## 7. Diagnostics

```text
stats() -> Statistics
diagnostics(request?: DiagnosticRequest, call?: CallOptions) -> Result<Diagnostic[]>
skips(request?: SkipRequest, call?: CallOptions) -> Result<Skip[]>
unresolved(request?: UnresolvedRequest, call?: CallOptions) -> Result<UnresolvedRef[]>
benchmark(request?: BenchmarkRequest, call?: CallOptions) -> Result<BenchmarkReport>
```

`unresolved` is deliberately public. "What does the graph not understand?" is a direct measure of extractor coverage, and it is how a user distinguishes "no such relationship" from "not indexed."

## 8. Errors and cancellation

Expected outcomes are statuses in the envelope ([Results and Evidence §5](results.md#5-error-taxonomy)). Only genuine execution faults raise.

Every potentially blocking operation accepts cancellation and a deadline through one consistent trailing-options shape:

```text
operation(request, call?: CallOptions) -> Result<T>
operation(call?: CallOptions) -> Result<T>  // when there is no domain request

CallOptions
  signal?:     CancellationSignal
  timeoutMs?:  Count                  // relative duration
  deadlineAt?: Timestamp              // absolute deadline
  progress?:   ProgressSink
```

If both timeout and absolute deadline are supplied, the earlier bound wins. The trailing `CallOptions` is execution control, never embedded in or persisted with the domain request. In-process bindings map native cancellation objects into `signal`; service transports map cancellation IDs/disconnects and wire deadlines into the same semantics without exposing transport types to the core. This contract applies to activation, updates, rebuilds, retrieval, context construction, diagnostics that may scan, benchmarks, and watch startup. Cheap in-memory accessors such as already-cached capability/status reads may omit it.

- Cancellation propagates to every port involved.
- Cancelled work is discarded, never partially committed.
- A cancelled operation returns `CANCELLED`; it does not raise.
- The engine enforces its own timeouts and never relies on a caller to bound it.

## 9. Stability

Semantic versioning applies to this document, not to any implementation's internals.

**Minor (compatible):**

- adding an operation
- adding an optional request field
- adding a response field
- adding a status, error code, kind, strategy, or capability flag

Clients MUST tolerate unknown values in open positions — statuses, codes, kinds, capability flags — rather than failing on them. This is what allows the engine to grow without coordinated client releases.

**Major (breaking):**

- removing or renaming an operation or field
- changing a field's type or meaning
- making an optional request field required
- removing a status or error code
- tightening a documented guarantee

**Never guaranteed:**

- the internal format of `EntityId`, `NodeId`, `EdgeId`, or `Revision`
- the on-disk layout
- ranking scores as absolute values (order is contracted; magnitudes are not)
- the exact set of nodes a given extractor produces at a given version

The opacity of identifiers and revisions is a deliberate purchase of future freedom. A client that parses them converts an internal detail into a compatibility obligation, and the scheme will change.
