# Storage

The persistence ports and the rules that govern them. Three separate ports, because they have genuinely different requirements and are frequently satisfied by different technologies — and because collapsing them forces every deployment to accept the union of their constraints.

```text
Store     transactional graph facts        required for graph capabilities
Lexical   text search index                optional; degrades to direct scan
Vector    nearest-neighbour retrieval      optional; semantic retrieval only
```

Nothing above L0 names a storage product, a query language, or a schema. A rule that cannot be expressed against these ports does not belong in core logic.

## 1. Store port

```text
Store
  open(location, options)   -> StoreHandle
  begin(mode)               -> Transaction
  read()                    -> ReadView
  versionRecords()          -> VersionRecords
  capabilities()            -> StoreCapabilities
  checkpoint()              -> Result           // WAL truncation & store compaction (ADR-019)
  close()

Transaction
  expectRevision(base: Revision)
  putNodes(claims: NodeClaim[])
  putEdges(claims: EdgeClaim[])
  removeNodeClaims(keys: FactClaimKey[])
  removeEdgeClaims(keys: FactClaimKey[])
  removeClaims(owner: FactOwner)
  removeByFile(root, path)
  putUnresolved(refs: UnresolvedRef[])
  removeUnresolved(keys: UnresolvedKey[])
  putSkips(skips: Skip[])
  putDiagnostics(diagnostics: Diagnostic[])
  putUpdateSummary(summary: UpdateSummary)
  putVersionRecords(records: VersionRecords)
  putIndexIntent(intent: DerivedIndexIntent)
  acknowledgeIndex(kind: lexical | vector, revision: Revision)
  setRevision(revision: Revision)
  commit()  -> Result
  rollback()

ReadView
  node(id)                     -> Node?
  nodesByName(name, filters)   -> Node[]
  nodesByFile(root, path)      -> Node[]
  edgesFrom(id, filters)       -> Edge[]
  edgesTo(id, filters)         -> Edge[]
  incomingEdgeCount(id)        -> Count
  unresolvedSeeking(name)      -> UnresolvedRef[]
  skips(filters)               -> Skip[]
  diagnostics(filters)         -> Diagnostic[]
  changesSince(revision)       -> ChangesResult
  versionRecords()             -> VersionRecords
  indexStates()                -> DerivedIndexState[]
  revision()                   -> Revision
```

`NodeClaim`/`EdgeClaim` pair a fact with its `FactOwner`; `FactClaimKey` pairs the canonical fact ID with that owner. The operation set is deliberately small and set-oriented. Facts are stored as owner-scoped claims under the `FactOwner` contract in [Graph Model §3](graph-model.md#3-provenance), then deterministically materialized into canonical nodes/edges. `removeClaims(owner)` removes only one producer/version's nodes, edges, unresolved references, skips, and diagnostics for a file. `removeByFile` removes all claims rooted at the previous file version, but not incoming edges claimed by references in other files; affected incoming claims are explicitly demoted through `removeEdgeClaims`/`putUnresolved`. Global removal by a bare canonical fact ID is intentionally absent: if another valid claim supports the same fact, materialization retains it. `expectRevision` makes an update plan conditional on the snapshot from which it was computed, preventing a stale plan from committing. Immutable normalized change summaries are persisted atomically with their graph revision so `changesSince` remains correct across reopen.

Two things are notable by their absence:

- **No general query language.** Traversal, ranking, and path-finding are core logic operating over these primitives, not delegated to the store. Delegating them would make core behavior depend on a store's query semantics, which is how a port becomes a dependency.
- **No per-item fact write.** Every fact write takes a batch ([§4](#4-write-discipline)). Revision, version, and derived-index acknowledgement records are transaction metadata and may be singular.

`ReadView` is a consistent snapshot. A view opened before a commit continues to observe the pre-commit state until released, which is what gives queries the isolation guarantee in [Incremental Updates §5](incremental.md#5-transactions).

### Required capabilities

A conforming store MUST provide:

- **Atomic multi-statement transactions.** Non-negotiable. Partial visibility of an update breaks the model.
- **Snapshot isolation for readers.** Readers observe one revision, never a mixture.
- **Durable commit.** A committed revision survives process termination: a successful return from a write call is the acknowledgement, and an acknowledged commit is never lost to process death. A crash of the OS or a power loss may lose a tail of the most recent commits, but must never leave torn or partially visible state, and the resulting lag against the working tree must be detectable so the affected files can be reindexed.
- **Efficient lookup by node id, by name, and by owning file.**
- **Efficient edge traversal in both directions.** Reverse traversal is not optional; impact analysis is entirely reverse traversal.
- **Efficient removal by owner claim and of all claims owned by one file.** This is the hot path of file replacement and scoped producer invalidation.
- **Conditional commit against a base revision.** A plan computed from revision A cannot commit over revision B.
- **Durable bounded change history.** `changesSince` remains complete across reopen or returns `TooOld`; it never returns a partial delta.

### Negotiated capabilities

```text
StoreCapabilities
  transactionalDDL:   bool
  concurrentReaders:  bool
  vectorsNative:      bool
  lexicalNative:      bool
  maxBatchSize?:      Count
  supportsSavepoints: bool
```

The engine adapts. Absent `concurrentReaders`, it serializes reads against the writer and reports reduced concurrency rather than failing. Absent native lexical or vector support, it uses the separate ports.

## 2. Single writer

**Exactly one authoritative writer per project graph, with concurrent readers.**

- The per-user global daemon owns the project writer lock for each active
  context. Its singleton daemon lease and each project's writer lock are
  separate exclusion scopes; holding the daemon lease does not authorize a
  write to any project that has not acquired its own lock.
- Project writer ownership is enforced by an atomic inter-process exclusion
  mechanism with operating-system release on process/handle death. A metadata
  record exposes holder identity, start time, and engine version for
  diagnostics, but metadata alone is never proof of ownership.
- A stale metadata record is reclaimable only after the exclusion mechanism is
  successfully acquired. PID checks, timestamps, and deleting a lock file are
  not sufficient—they race with live holders and PID reuse.
- Clients never acquire, release, or delete `.repin/writer.lock`. If the
  daemon cannot claim it because another process owns it, it attaches an
  observer where safe and reports `PROJECT_LEASE_UNAVAILABLE` for graph
  writes. Direct working-tree retrieval remains available and the daemon does
  not buffer writes it cannot commit.
- Reader and observer clients observe durable graph progress by comparing
  revisions for equality. They do not require a revision event stream or
  session state, which keeps reconnecting and short-lived clients correct; an
  individual operation may still receive the advisory API progress events
  defined in [Public API — Progress events](api.md#progress-events).
- Semantic indexing is scheduled through the same **writer coordinator**, so its mutation cannot interleave with a deterministic commit. The vector port may own a separate physical index and transaction; “same writer” means serialized ownership and deterministic commits take priority, not that graph and vector storage share one transaction.

This constraint is implemented by the user daemon and its isolated project
contexts ([Architecture §11](architecture.md#11-deployment-topologies)), not by
several client processes contending for the same store. Multiple connections
to one canonical database share the daemon's context; different canonical
database paths have independent locks, revisions, watchers, and indexes.

## 3. Version records

Persisted alongside the graph:

```text
VersionRecords
  storeSchemaVersion:      Version
  kindRegistryVersion:     Version
  attributeRegistryVersion: Version
  classificationVersion:   Version
  resolutionVersion:       Version
  packVersions:            Map<LanguageId, Version>
  extractorVersions:       Map<ExtractorId, Version>
  engineVersion:           Version
  vcsRevision?:            Text
  observedDirtySet?:       Path[]
```

On open:

| Condition | Action |
|---|---|
| all versions match | reuse the graph |
| older store schema, migration available | migrate in one transaction |
| older store schema, no migration | rebuild |
| kind or attribute registry changed | migrate or rebuild per change |
| classification rules changed | reclassify affected files |
| resolution rules changed | re-resolve; no re-parse needed |
| a pack or extractor version changed | re-extract only that extractor's files |
| newer than the engine understands | **refuse to open** |

Two rules carry the weight here:

**Scoped invalidation.** A version change invalidates only the facts its owner produced. Upgrading one language pack re-extracts that language; it does not rebuild the graph. This is only possible because provenance records the producing extractor and version ([Graph Model §3](graph-model.md#3-provenance)).

**Refuse rather than corrupt.** A graph written by a newer engine is not readable by an older one. Opening it read-only and hoping is how stores get corrupted; refusing is a recoverable inconvenience.

A full rebuild is always an acceptable fallback. Serving facts produced by a different extractor version is not.

## 4. Write discipline

**Never one statement per item.**

```text
avoid:    for each node: write(node)
prefer:   write(nodes[0..n])
```

Per-item writes are the worst pattern for every store, and dramatically worse for stores with higher per-operation overhead — which includes anything reached over a boundary or a socket. The overhead is per *operation*, not per row, so batching amortizes it directly.

- Insert in multi-item batches. Moderate batch sizes capture nearly all the available gain; very large batches add memory pressure without improving throughput.
- Keep a small set of pre-prepared batch shapes rather than constructing a new one per batch size.
- Prefer positional over keyed row reads on hot paths where the port offers both; per-row object construction is measurable at scale.
- Build secondary indexes **after** the initial bulk load, not during.
- One transaction per logical update, not per file.

Concrete figures belong in the implementation profile. The portable rule is that per-operation overhead dominates bulk work, so bulk work must be expressed in bulk operations.

## 5. Lexical port

```text
Lexical
  index(documents: LexicalDoc[])   // batched
  remove(keys: DocKey[])
  query(request: LexicalQuery) -> LexicalHit[]
  capabilities() -> LexicalCapabilities
  close()

LexicalQuery
  text:      Text
  mode:      terms | phrase | prefix | regex
  filters:   { roots?, paths?, languages?, artifactClasses?, nodeKinds? }
  limit:     Count

LexicalHit
  key:       DocKey        // file or node
  score:     Score
  regions:   Range[]       // where matches occurred
  snippet?:  Text
```

Requirements: batched indexing, incremental removal by key, filtered query, and match regions. Regions are required rather than optional because evidence needs a location ([Results and Evidence §2](results.md#2-evidence)) — a hit without a location is not actionable.

```text
LexicalCapabilities
  phrase:   bool
  prefix:   bool
  regex:    bool
  ranked:   bool
  snippets: bool
  filters:  FilterKind[]
```

**Absence is survivable.** With no lexical port, text search falls back to a direct bounded scan of the working tree: slower, less well ranked, still correct. This is what makes [Architecture §1](architecture.md#1-two-capabilities-one-product)'s "direct retrieval MUST work with no graph" true in the degraded case too.

Indexed content is subject to the same exclusion and redaction rules as returned output ([Safety and Data Handling](safety.md)). An excluded file is never indexed, so it cannot leak through a snippet.

### Lexical evidence verification

The working tree is authoritative for returned evidence. A lexical hit's
stored region and snippet are an index hint, not proof that the current file
still contains the match. Before a lexical region is exposed, the engine
re-reads the selected root-relative file through the normal safety boundary,
checks its current tagged hash against the indexed document identity, and
verifies the match at the returned byte range. A changed hash, invalid range,
failed read, or failed verification drops that lexical evidence and triggers a
current direct scan when the budget permits. The result reports the lexical
state and coverage; it never presents a stale snippet as current.

An implementation MAY use an index-derived region as a fast path, but the
conformance fixture includes a re-read-and-verify path and a deliberate stale
index case. This keeps the evidence source explicit while allowing the S2
future adapter work to measure whether verification can be safely elided for
any bounded internal operation.

## 6. Vector port

```text
Vector
  upsert(entries: VectorEntry[])
  remove(keys: VectorKey[])
  search(query: VectorQuery) -> VectorHit[]
  capabilities() -> VectorCapabilities

VectorEntry
  key:       VectorKey       // node id plus chunk ordinal
  embedding: Embedding
  metadata:  { root, language?, nodeKind?, artifactClass? }

VectorQuery
  embedding: Embedding
  filters:   MetadataFilters
  limit:     Count
```

Requirements: upsert and delete by key, filtered nearest-neighbour search, and dimension consistency enforcement.

Rules:

- Entries are keyed by node id plus chunk ordinal, so a large node's several chunks are individually addressable and collectively removable.
- **An acknowledged deletion must be externally complete.** After `remove` succeeds, and after reopen, the deleted key MUST NOT appear in search results. An implementation may use internal tombstones only if it filters them correctly and eventually compacts them; internal representation is not part of the port contract.
- **Metadata filtering is required.** Unfiltered semantic search over an entire repository is rarely what a caller wants; filtering by root, language, or kind is what makes it useful.
- Dimension and metric are fixed at index creation. Changing either invalidates the index, which is why they participate in the embedding cache key ([Optional Intelligence](intelligence.md)).
- Absence disables semantic retrieval and nothing else. Deterministic retrieval is unaffected.

The accepted I5 baseline in
[ADR-012](decisions/ADR-012-exact-rust-vector-baseline.md) stores derived
embedding rows and metadata in SQLite, filters them in SQL, and streams vectors
through Rust distance computation with a bounded top-k heap. This physical
co-location does not make semantic updates synchronous with graph commits.

## 7. Consistency between indexes

Three stores that can disagree, and defined behavior when they do.

| Index | Advances | On lag |
|---|---|---|
| graph | on every authoritative store commit | authoritative |
| lexical | transaction-coupled when native; otherwise immediately after graph commit | bypassed; current direct scan substitutes where supported |
| vector | asynchronously, always | stale content possible |

Rules:

- The **graph is authoritative**. Where an index disagrees with it, the graph wins and the index is repaired.
- A lexical implementation participating in the authoritative store transaction advances atomically with the graph. A separate lexical port cannot be assumed to share that transaction. In that case the graph commit records the desired lexical revision or reconstructable pending work, the lexical commit follows, and completion is acknowledged afterward. Interruption at any boundary leaves a detectable lag that is repaired idempotently or rebuilt.
- A successful authoritative graph commit is never rolled back merely because a separate derived-index commit fails. The update remains usable through graph and direct channels; status and result warnings report the lexical bypass until repair.
- A known-lagging lexical index is not queried for user results. The retrieval coordinator substitutes a current working-tree scan for lexical modes where direct search supports the request, then merges graph channels deterministically. If that fallback covers the full requested scope, lag is warned but coverage may remain complete; scan/resource limits, skips, timeout, or an unsupported direct mode make the result `partial` with reduced coverage. Repair runs independently.
- The vector index is **always** asynchronous. A deterministic revision MUST NEVER wait for embedding work.
- Every index reports its own revision and state. Status exposes graph, lexical, and vector revisions plus pending counts where known, so a caller sees lag rather than inferring it from surprising results.
- Unknown lag is handled defensively: any index hit resolving to a graph entity that no longer exists is **dropped**, not returned. Known stale lexical content is never presented as current evidence.

ADR-012 selects reconstructable pending work plus a later vector-table commit
and semantic-revision acknowledgement for the initial vector profile. Crash
observability, idempotent recovery, and graph authority remain portable
requirements.

## 8. Migration

- Migrations run in one transaction, or are resumable with a recorded progress marker. A migration that can half-apply is a corruption vector.
- Every migration has a rebuild fallback. Where a migration is complex or risky, rebuilding is the correct choice — the graph is derived data, and rebuilding is always available.
- Migration never loses skip records or diagnostics. Losing them silently improves apparent coverage.
- Revisions are not reused across migration ([Incremental Updates §10](incremental.md#10-revisions)).
- Enrichment facts (`derivation: inferred`) are **discardable during migration**. Migrating them is optional; regenerating them is always acceptable and usually cheaper.

### Migration ownership

Project configuration and graph state have separate schemas, owners, and
failure boundaries:

| Artifact | Version owner | Migration boundary | Failure behavior |
|---|---|---|---|
| user/trusted-project configuration | configuration loader/composition root | parse and migrate in memory before engine activation | leave the source file unchanged; refuse activation with a configuration diagnostic if it cannot be migrated or validated |
| graph store and its durable metadata | store adapter plus engine version records | after roots/configuration are validated and the writer is acquired | run transactionally or resumably with a marker; fall back to rebuild without exposing a half-migrated graph |
| kind/attribute/classification/resolution/pack versions | engine domain and language-pack registry | scoped invalidation/re-extraction or re-resolution, not config migration | preserve the prior authoritative revision until the replacement plan commits |
| lexical/vector indexes | their derived-index adapters | independent rebuild/repair from authoritative graph/content | leave graph revision valid; report lag and pending work |

Activation follows this order: load configuration layers and migrate them in
memory; validate roots and state-directory permissions; acquire the project's
writer lock or enter an explicitly reported observer/direct-only mode; inspect
store version records; then migrate/rebuild the store and schedule derived
indexes. Configuration migration never edits graph tables, and store migration
never rewrites user configuration.

An implementation MAY offer an explicit `config migrate` or equivalent CLI
operation to rewrite a project file, but ordinary activation is read/validate
only. Unknown configuration fields are preserved by a migration that can do so
and otherwise produce a diagnostic; unknown store schema is refused rather
than guessed. This separation lets a configuration rollback and a graph
rebuild be operated independently. Deleting `.repin` remains a complete
recovery action only after the active project context has unloaded; active
deletion is treated as an identity change and fails that context closed.

## 9. State on disk

- All durable project state lives under `.repin`, self-ignored so derived
  artifacts are never committed. The initial layout is:

  ```text
  project/.repin/
    graph.sqlite3
    writer.lock
  ```

- **Deleting `.repin` is safe and sufficient only after its context has
  unloaded.** Nothing outside it is required to rebuild, but deleting active
  state is an identity change: the daemon fails the active context closed and
  requires a new initialization/activation cycle.
- State is not world-readable by default; it holds structural information and
  may hold content snippets. The per-user daemon socket and lease are outside
  `.repin` in the private runtime directory and are not durable project data.
- The layout is an implementation detail. No client reads it directly, and
  nothing outside the store/context adapter depends on physical index names.

The engine applies the permission and symlink/reparse-point checks in [Safety
and Data Handling §8](safety.md#8-state-on-disk) before the store adapter opens
any file. A store adapter may add stricter requirements, but it may not weaken
the engine's private-state floor or treat an unverifiable permission check as
safe.
