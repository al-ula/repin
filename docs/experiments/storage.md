# Storage Adapter Experiments

Disposable experiments used to evaluate the proposed Rust storage profile in [`docs/technology-candidates.md`](../technology-candidates.md).

```text
Status: planned
Lifecycle stage: planning
Execution stage: experimentation
Production code: no
```

## 1. Purpose

Planning chooses what to test; experimentation produces evidence. These spikes answer whether redb, Tantivy, and USearch can satisfy Repin's existing ports and recovery rules without changing the architecture to fit a product.

Spike code is disposable. Fixtures, workloads, methods, raw results, failure reproductions, and conclusions are retained. Passing a synthetic happy path is not enough: each experiment includes interruption, deletion, and reopen behavior where applicable.

## 2. Shared method

All experiments follow [`docs/conformance.md` §6](../conformance.md#6-benchmark-method). Record each run or comparable run group with [`docs/experiments/template.md`](template.md); retaining the template's environment, traceability, raw-evidence, failure, and limitation fields is mandatory even if results are rendered by another tool.

Record with every run:

- source revision and dependency versions
- operating system, architecture, Rust toolchain, and build profile
- CPU, memory, storage medium, filesystem, and free-space state
- fixture/corpus revision and composition
- cold or warm state
- concurrency and batch sizes
- run count, variance, and raw measurements
- state layout before and after interruption

Use seeded, reproducible workloads. Preserve any seed that exposes a failure as a regression case.

The experiments should use the corpus bands in [`docs/experiments/fixtures.md`](fixtures.md) and representative graph shapes, not only random key-value data:

- a small repository fixture for correctness inspection
- a medium mixed-artifact fixture for routine measurements
- a high-fan-in graph for reverse-impact traversal, from [`G-FANIN`](fixtures.md#generator-g-fanin-high-fan-in-s-003)
- a high-fan-out graph for neighbor traversal, from [`G-FANOUT`](fixtures.md#generator-g-fanout-high-fan-out)
- files with many owned facts for replacement cost, from [`G-REPLACE`](fixtures.md#generator-g-replace-file-replacement-s-004)
- a bulk-change fixture for batching and recovery

Every generated shape records its generator version, parameter tuple, and seed, and is compared against the generator's own oracle rather than against another candidate's output.

Performance thresholds are not invented during execution. If targets are not yet known, report distributions and scaling curves; plan finalization will set supported limits from evidence.

## 3. Experiment S1 — redb store adapter

### S1 candidate pin

Use the S-001 `redb` 4.1.0 pin, source record, checksum, declared Rust 1.89 baseline, and no-feature initial configuration in [Technology Candidates — S-001 experiment pin](../technology-candidates.md#s-001-experiment-pin). The disposable S1 workspace MUST declare the exact version, commit `Cargo.lock`, and record its lockfile SHA-256, active features, and toolchain in each result using the [Experiment Result Template](template.md). It MUST NOT infer the eventual Repin MSRV from this candidate's declared Rust version.

### S1 Questions

- Can redb provide atomic batch updates and stable snapshot readers?
- Are all required lookup and traversal paths efficient with explicit indexes?
- Can all facts owned by one file be replaced without a global scan?
- What happens on process interruption and reopen?
- How should daemon-owned project-writer ownership and observer/direct-only
  fallback be represented?

### S1 Prototype schema

The spike should model at least:

- node/edge claims by canonical id and `FactOwner`
- canonical nodes by id with deterministic claim materialization
- node IDs by S1 exact name and qualified-name lookup text
- claim keys by owning root/path/producer/version
- canonical edges by source
- canonical edges by target
- unresolved references by key and sought name
- file metadata and content identity
- version records and current revision
- immutable normalized update summaries, retained revision index, and history floor
- derived-index revisions or pending-work markers

The schema is experimental and need not match the final on-disk format.

### S1 experimental key encoding (S-002)

This is the reviewable key and secondary-index design for the disposable S1 redb spike. It implements the access paths required by the portable [Store port](../storage.md#1-store-port) and the owner-claim model in [Graph Model — Ownership claims](../graph-model.md#ownership-claims); it does **not** define Repin's production on-disk format, public identifier representation, root-identity policy, or final value codec. In particular, `RootId`, canonical IDs, producer IDs, and versions enter the encoder as already-canonical opaque byte sequences. The resolved root-identity policy (`C-002` in [Planning Task Backlog](../tasks.md)) remains outside S1, so the spike cannot accidentally redefine it.

#### Encoding rules

Every S1 table uses byte-string keys and a table-local value codec. A key begins with the one-byte encoding version `0x01`; changing its interpretation requires a new table name or a recorded S1 migration case, never mixed encodings in one table.

| Component | Encoding | Purpose |
|---|---|---|
| opaque atom (`RootId`, `NodeId`, `EdgeId`, producer ID, version, registered kind, digest) | `u32be(byte length) || bytes` | Delimits arbitrary bytes without sentinel escaping. |
| S1 text atom | valid UTF-8 bytes exactly as supplied, wrapped as an opaque atom | Keeps a displayed `Node.name`, `qualifiedName`, `seeking`, or path distinct from any unchosen global normalization policy. |
| fixed ordinal | `u64be` | Preserves numeric order for an ordinal or a spike-local revision. |
| fixed S1 enum | one assigned byte from the table below | Keeps table routing and singleton state unambiguous. |
| tuple | concatenation of its encoded components | Makes the encoding of an exact leading tuple a range-scan prefix. |

S1 applies no case folding, Unicode normalization, path normalization, or version-text interpretation inside this encoder. Callers must supply already validated root-relative paths and the extraction/resolution keys they intend to compare; the S1 fixture initially uses exact UTF-8 byte equality. It indexes `Node.name` and, where present, `qualifiedName` separately, rather than calling either a “normalized name.” This deliberately leaves global name, path, root, and version normalization to their respective contract decisions. IDs and hash digests remain opaque bytes. The narrow S1 file-value rule for a persisted `Hash` is `(algorithm atom, digest atom)`, never a digest whose algorithm is inferred; it does not prescribe the final value codec. The spike rejects an overlong atom before allocation; its exact maximum is a measured resource limit, not a schema constant. All key construction goes through one testable encoder—no hand-concatenated keys at call sites.

| Fixed S1 enum | Assigned bytes |
|---|---|
| `claimKind` | `node=0x01`, `edge=0x02`, `unresolved=0x03`, `skip=0x04`, `diagnostic=0x05` |
| `nameField` | `name=0x01`, `qualifiedName=0x02` |
| `indexKind` | `lexical=0x01`, `vector=0x02` |
| `stateKind` | `currentRevision=0x01`, `historyFloor=0x02`, `keySchemaMarker=0x03` |
| `recordKind` | `storeSchema=0x01`, `kindRegistry=0x02`, `attributeRegistry=0x03`, `classification=0x04`, `resolution=0x05`, `packVersions=0x06`, `extractorVersions=0x07`, `engine=0x08`, `vcsRevision=0x09`, `observedDirtySet=0x0a` |

The `revision` fields below use a big-endian `u64` only inside S1. The portable `Revision` remains opaque, and a production adapter may use another monotonic representation. `keySchemaMarker` stores the S1 table/key encoding version (`0x01`); it is distinct from `recordKind.storeSchema`, which stores the portable `VersionRecords.storeSchemaVersion` input used by open/migrate/rebuild tests.

`UnresolvedId` is not a hash: it is the S1 opaque byte encoding `0x01 || FactOwner || from || seeking || scopeHintPresent || scopeHint when present || edgeKind || occurrence`. `scopeHintPresent` is `0x00` or `0x01`, so absent and present-empty hints cannot collide. `occurrence` is `0x00 || u64be(fixtureOccurrenceOrdinal)` when no provenance range exists, otherwise `0x01 || u64be(start byte offset) || u64be(end byte offset) || u64be(fixtureOccurrenceOrdinal)`; S1 fixtures assign a deterministic, zero-based ordinal in extractor output order to every otherwise-identical occurrence. Thus two occurrences from one owner/node with identical lookup text do not silently collapse, including range-less fixture references. `SkipId` and `DiagnosticId` are each a fixture-assigned `u64be` ordinal scoped to `FactOwner`; their primary values remain the source of record content. These identifiers and their encodings are S1-local, are not public IDs, and create no new graph-model identity rule.

#### Tables and access paths

Values carry the corresponding normalized fact, metadata, or index payload; key-only index rows use an empty value. A mutation updates its primary row, canonical materialization, and every listed secondary row in one redb write transaction. A failed transaction leaves all of them at the preceding revision.

| redb table | Key tuple after `0x01` | Value / use | Required operation |
|---|---|---|---|
| `s1_node_claims_v1` | `(NodeId, FactOwner)` | `NodeClaim` | Read all claims for a canonical node; replace a known claim. |
| `s1_edge_claims_v1` | `(EdgeId, FactOwner)` | `EdgeClaim` | Read all claims for a canonical edge; replace a known claim. |
| `s1_unresolved_claims_v1` | `(UnresolvedId, FactOwner)` | `UnresolvedRef` | Remove/promote/demote one owner-scoped unresolved reference. |
| `s1_skips_v1` | `(SkipId, FactOwner)` | `Skip` | Remove one owner-scoped skip. |
| `s1_diagnostics_v1` | `(DiagnosticId, FactOwner)` | `Diagnostic` | Remove one owner-scoped diagnostic. |
| `s1_claims_by_file_v1` | `(RootId, Path, producer, producerVersion, claimKind, targetId)` | empty | Prefix scan for `removeClaims(owner)` and `removeByFile(root, path)`; routes `targetId` to its primary table according to `claimKind`. |
| `s1_nodes_v1` | `(NodeId)` | deterministically materialized `Node` | `node(id)`. |
| `s1_nodes_by_name_v1` | `(nameField, exactUtf8Text, NodeId)` | empty | Prefix scan for `nodesByName`; `nameField` distinguishes written `name` from optional `qualifiedName`, and the canonical node is then read by ID. |
| `s1_nodes_by_file_v1` | `(RootId, Path, NodeId)` | empty | Prefix scan for `nodesByFile`; contains only materialized current nodes. |
| `s1_edges_v1` | `(EdgeId)` | deterministically materialized `Edge` | Read canonical edge by ID while traversing. |
| `s1_edges_by_from_v1` | `(fromNodeId, EdgeId)` | empty | Prefix scan for `edgesFrom`. |
| `s1_edges_by_to_v1` | `(toNodeId, EdgeId)` | empty | Prefix scan for `edgesTo`; reverse traversal never scans all edges. |
| `s1_unresolved_by_seeking_v1` | `(exactUtf8Seeking, UnresolvedId, FactOwner)` | empty | Candidate prefix scan when a definition is added; S1 supplies the same exact lookup text for a definition and seeking reference. |
| `s1_promoted_references_v1` | `(EdgeId, FactOwner, UnresolvedId)` | original `UnresolvedRef`, resolution input/result, and deterministic occurrence aggregation | Prefix scan by edge retains enough owner-scoped data to demote a promoted edge back to every original unresolved occurrence. |
| `s1_promoted_by_seeking_v1` | `(exactUtf8Seeking, FactOwner, UnresolvedId, EdgeId)` | empty | Candidate prefix scan to re-resolve both unresolved and already promoted occurrences when definitions change. |
| `s1_promoted_by_owner_v1` | `(RootId, Path, producer, producerVersion, EdgeId, UnresolvedId)` | empty | Prefix scan removes every promoted-reference sidecar owned by a file/producer without scanning all edges. |
| `s1_skips_by_file_v1` | `(RootId, Path, SkipId, FactOwner)` | empty | Prefix scan for `skips(filters)` and owner/file removal. |
| `s1_diagnostics_by_file_v1` | `(RootId, Path, DiagnosticId, FactOwner)` | empty | Prefix scan for `diagnostics(filters)` and owner/file removal. |
| `s1_files_v1` | `(RootId, Path)` | current file metadata, tagged content hash using the narrow S1 value rule, classification, and last committed revision | Deduplication, file replacement, and reopen checks. |
| `s1_update_summaries_v1` | `(revision)` | immutable normalized `UpdateSummary` | `changesSince`; written with graph facts and current revision. |
| `s1_history_revisions_v1` | `(revision)` | empty | Detect a missing history summary before returning a delta. |
| `s1_state_v1` | `(stateKind)` | current revision, history floor, and schema marker | Singleton durable state records. |
| `s1_index_state_v1` | `(indexKind)` | acknowledged revision, target revision, status, and pending count | Report lexical/vector freshness. |
| `s1_pending_work_v1` | `(indexKind, targetRevision, workOrdinal)` | reconstructable derived-index work | Prefix scan by index/revision during idempotent recovery. |
| `s1_version_records_v1` | `(recordKind)` | one [VersionRecords](../storage.md#3-version-records) field or a canonical aggregate | Open/migrate/rebuild/refuse decisions. |

`FactOwner` is encoded exactly as `(RootId, Path, producer, producerVersion)` in every occurrence. `claimKind` determines the `targetId` and primary row: `node → NodeId`, `edge → EdgeId`, `unresolved → UnresolvedId`, `skip → SkipId`, and `diagnostic → DiagnosticId`. The skip and diagnostic primary/index rows are present from the first S1 schema so that `putSkips`, `putDiagnostics`, `ReadView.skips`, `ReadView.diagnostics`, and `removeClaims(owner)` exercise the same ownership rule as nodes and edges.

For S1, `nodesByName(text, filters)` prefix-scans both `(name, text)` and `(qualifiedName, text)`, unions and deduplicates `NodeId`s, reads canonical nodes, then applies port filters in core/spike logic. A newly defined node supplies its written `name` and, if present, its `qualifiedName` as separate exact lookup texts to `s1_unresolved_by_seeking_v1`; a fixture reference declares which exact text it seeks. This is deliberately narrower than choosing global normalization or name-resolution policy. `skips(filters)` and `diagnostics(filters)` use the root/path prefix when supplied; other S1 filter fields are applied after reading those bounded rows, or after a full respective-primary-table scan when no root/path is supplied. Their measured cost is reported rather than represented as a required optimized access path.

#### Mutation and verification rules

1. **Put claims:** upsert primary claim rows, then materialize only affected canonical nodes/edges. Replace the corresponding name, file, forward, and reverse rows rather than relying on redb iteration order.
2. **Remove claims:** prefix-scan `s1_claims_by_file_v1` for the exact owner or file prefix and `s1_promoted_by_owner_v1` for the same scope; delete its primary claim, canonical/index rows, every promoted-reference sidecar, and every promoted-owner/promoted-seeking index row in the same transaction; rematerialize each affected canonical fact from remaining claims. A fact with another valid owner remains materialized.
3. **Resolve references:** `s1_unresolved_by_seeking_v1` and `s1_promoted_by_seeking_v1` are candidate lookups only. Whenever a definition is added, removed, or changes a lookup/scope-relevant field, S1 unions the unresolved and promoted candidate rows for each affected S1 lookup text, then applies its explicit fixture resolution predicate: the exact lookup text must match, a present `scopeHint` must match the fixture's allowed definition scope, the source scope must permit the candidate, and exactly one candidate must remain after the fixture's deterministic ordering `(qualified lookup text, RootId bytes, Path bytes, NodeId bytes)`. Zero candidates or ambiguity leaves an occurrence unresolved; it never creates multiple edges merely because a prefix scan found multiple rows. For one selected target, S1 atomically removes each selected unresolved primary/index row, writes `(EdgeId, FactOwner, UnresolvedId)` to `s1_promoted_references_v1` and its owner/seeking index rows with the full original reference and resolution result, and adds the owner-scoped edge claim plus forward/reverse rows. For a previously promoted occurrence whose selected target changes, becomes absent, or becomes ambiguous, S1 removes its old sidecar and owner/seeking index rows, removes the associated edge claim and canonical/index rows only if no other sidecar supports that `(EdgeId, FactOwner)`, and restores its original unresolved primary/index row before applying any newly selected target. An edge claim represents the canonical relationship; multiple same-owner occurrences that yield that edge retain one sidecar each and a canonical deterministic aggregation ordered by `UnresolvedId`. When a target node ceases to be materialized, S1 uses `s1_edges_by_to_v1` then the edge-key prefix of `s1_promoted_references_v1` to find **every** affected promoted reference. It removes every corresponding edge claim, canonical/index row, promoted sidecar, and promoted-owner/promoted-seeking row, then restores every original unresolved primary/index row. No claim may preserve an edge to a non-existent target. S1 cases include two same-owner occurrences through promote/delete/re-add and `reference → definition A → competing definition B → remove B`; a surviving edge alone is insufficient evidence of correct demotion or re-resolution.
4. **Update file state:** in that same transaction, create/modify replaces `s1_files_v1` only after the associated claims are ready; delete removes its row; rename removes the old `(RootId, Path)` row and writes the new one. Deduplication compares a proposed create/modify only to a present current row at that exact root/path, never to deleted state.
5. **Commit revision and index intent:** a graph commit writes changed canonical rows, one normalized update summary, its revision-index row, current revision, and an S1 derived-index intent in the same transaction. For each enabled derived index, intent sets `targetRevision` to the new graph revision and replaces all older pending rows with a **full, idempotent rebuild descriptor from the authoritative graph at `targetRevision`**, not a delta whose base may have been partially applied. A no-op writes none of them.
6. **Acknowledge/recover derived work:** an index acknowledgement is accepted only for an existing graph revision no greater than its recorded target and no less than the recorded acknowledged revision. It records the completed revision and clears only `s1_pending_work_v1` rows matching `(indexKind, completedRevision)`; unknown, future, or regressive acknowledgements are rejected. If newer target work exists, it remains pending and the index state remains lagging; failure clears nothing. On reopen, S1 compares graph/current revision, target, acknowledgement, and remaining rows, then rebuilds the derived index from authoritative graph state for the newest pending target. This is an S1 coalescing choice to test, not the final derived-index protocol.

S1 must assert after every mutation that every canonical node/edge has the expected secondary rows, every index row resolves to an existing canonical record, every claim-index row resolves to one owner-scoped primary claim, and all prefix removals remain file/owner bounded. It must inspect the redb table list and sample encoded keys after reopen to confirm that no unsupported redb ordering or implicit key representation is being relied upon. The required behavioral tests remain [S1 Cases](#s1-cases); this design is a hypothesis to falsify, not an accepted schema.

### S1 Cases

1. Insert a complete graph in moderate batches and reopen it.
2. Replace all claims for one file in one transaction; remove one of two producers claiming the same canonical fact and verify the other claim/materialized fact survives.
3. Read outgoing and incoming edges on high-degree fixtures.
4. Add and remove a definition while promoting and demoting unresolved references.
5. Hold a read view across a concurrent commit; verify it observes exactly one revision.
6. Abort before commit; verify no facts or revision become visible.
7. Terminate the process, and separately inject each supported file fault, at every `(point, fault)` pair in [S1 durability and crash injection](#s1-durability-and-crash-injection-s-005); reopen and verify that point's expected durable state and every recovery assertion.
8. Attempt two project contexts using an OS-backed lock candidate (for example `fs4`); verify one authoritative owner and an observable observer/direct-only outcome for the other context. Kill the holder, retain stale metadata, reacquire safely, and test a filesystem without reliable lock support.
9. Persist normalized summaries atomically with commits; exercise `changesSince` across reopen, inject a missing summary, and compact by count/age while verifying the history floor and `TooOld` metadata.
10. Invalidate a prepared plan twice through revision/content churn; verify no stale commit, bounded retry, `UPDATE_CONFLICT`, and scheduled reconciliation.
11. Apply a version mismatch and exercise migrate, rebuild, and refuse-newer paths.
12. After unloading the project context, delete `.repin` and rebuild using only
    repository inputs; separately verify that deleting active state fails the
    context closed rather than continuing through stale handles.

### S1 durability and crash injection (S-005)

S1 Case 7 is only reproducible if every termination point and its expected durable state are named in advance. This section defines them. It is an experiment method, not a new store contract.

#### Durability target

| Failure | Requirement |
|---|---|
| Process termination (SIGKILL) at any point | Reopen yields a well-formed committed revision. Every commit whose write call returned success is present. |
| OS crash or power loss | Reopen yields a well-formed committed revision. Losing a tail of the most recent commits is acceptable, provided nothing is torn and the resulting lag against the working tree is detectable. |
| Any failure | No mixed, partial, or torn state is ever observable: no canonical fact without its secondary rows, no summary without its revision, no revision without its facts. |

A successful return from the store's write call **is** the acknowledgement. A commit that returned success must therefore be present after process termination; the caller crashing before it acts on that return value does not un-commit anything.

The floor is therefore **crash consistency with a permitted commit tail loss on OS or power loss**, not per-commit power-loss durability. This is deliberate: the graph store is authoritative for persisted facts but rebuildable from the working tree, so a lost tail is recoverable by reindexing while a torn store is not recoverable at all. A candidate that can only offer crash consistency by disabling its own integrity checks fails.

No numeric loss bound is asserted during planning. S1 **measures** the observed tail loss at every fault point — how many returned commits and which files were lost — and reports it. The supported bound is an evidence-based decision at plan finalization, not a threshold invented now. What is a pass condition is that the loss is detectable: after reopen, comparing every persisted `s1_files_v1` content identity against the working tree must yield exactly the expected set of files needing reindexing, and no lost file may appear current.

#### Injection mechanism

Two mechanisms, both seeded and both required:

1. **Process termination.** A parent harness runs the spike as a child, waits for the child to report that it reached a named point, then sends `SIGKILL` or the platform equivalent. No destructor, unwind, flush, or drop may run. The harness never terminates itself, so the surviving process can assert on the state.
2. **Fault-injecting file wrapper.** The spike reaches its state directory through a wrapper that can inject, at a named point, a short write, an fsync failure, `ENOSPC`, or an `EIO` read failure. This separates "the application recovered" from "the storage engine's assumptions held," which termination alone cannot distinguish. Injection points are addressed by `(pointName, occurrenceOrdinal)` so a case is rerunnable exactly.

Real power-loss testing on dedicated hardware is explicitly **out of scope for Stage 2**. It is recorded as a limitation in every affected result and as a candidate release-qualification activity rather than an experiment gate.

#### Named termination and fault points

Points are stable names; a result cites the point name and the injected fault. `P1`–`P4` precede any durable effect, `P5`–`P9` bracket the commit, `P10`–`P13` follow it, and `P14` covers a failing read after reopen. "kill" means process termination.

| Point | Position | Injectable faults | Expected durable state after reopen |
|---|---|---|---|
| `P1` | after opening the store, before any write transaction | kill | previous revision intact; no new revision |
| `P2` | after the writer lock is acquired, before the first write | kill | previous revision intact; lock reacquirable without manual cleanup |
| `P3` | mid-batch, after some claim rows are written | kill, `ENOSPC` | previous revision; no partial claim visible |
| `P4` | after all claim rows, before canonical materialization | kill, `ENOSPC` | previous revision; no canonical or index row from this update |
| `P5` | after materialization, before the update summary is written | kill, `ENOSPC`, short write | previous revision only; nothing from this update is visible |
| `P6` | after the summary, before the current-revision record | kill, `ENOSPC`, short write | previous revision only; a summary for an uncommitted revision is invisible or discarded, never reported by `changesSince` |
| `P7` | after the current-revision record, before derived-index intent | kill, short write | previous revision only, because all four records share one transaction; if the new revision is visible, its summary **and** its derived-index intent must both be present |
| `P8` | inside the candidate's own commit/fsync path | kill, fsync failure, short write | previous or new revision, never a mixture; a torn commit is a failure |
| `P9` | after the commit call returns successfully | kill | the new revision, because a returned commit is acknowledged; reopen presents it as complete |
| `P10` | after commit, during derived-index reconciliation | kill, `ENOSPC` | new graph revision; pending derived work still present and replayable |
| `P11` | after derived work completes, before acknowledgement | kill, `ENOSPC` | new graph revision; derived index lagging; rebuild is idempotent |
| `P12` | during whole-revision history compaction | kill, `ENOSPC` | contiguous history or a raised floor with `TooOld`; never a gap that reports as contiguous |
| `P13` | during migration or rebuild | kill, `ENOSPC`, fsync failure | the previous valid state, or a recorded resumable migration/rebuild marker whose retry or full rebuild completes; see the `P13` exception below |
| `P14` | first read after reopening following `P8` | `EIO` | a structured read error; never a silently truncated or partial graph |

`P5`–`P7` all expect the previous revision alone: mutation rule 5 puts the canonical rows, summary, revision-index row, current revision, and derived-index intent in one redb transaction, so a pre-commit interruption cannot publish a subset. These points exist to falsify that claim, not to permit partial states.

**`P13` exception.** `P13` is the only point where reopen may refuse to serve. Migration and rebuild are explicitly resumable in [Storage §8](../storage.md#8-migration), so an interrupted migration must leave either the previous valid state or a durable marker identifying the in-progress migration and its restart point. A refusal must be a structured error naming the state and the available recovery action, and the case then asserts that retrying the migration or performing a full rebuild reaches a well-formed revision. A refusal without a marker or without a viable recovery action is a failure.

Every `(point, fault)` pair in the table is exercised at least once on the
Linux x86_64/glibc PoC target. Non-Linux and lower-tier platform runs are not
part of the current storage plan; platform-specific recovery work starts in
the post-PoC expansion phase.

#### Expected recovery behavior

After every injection the spike reopens and asserts, in this order:

1. Reopen succeeds, or — only at `P13` or `P14` — fails with a structured error naming the state and the available recovery action. A silent partial open is a failure at every point.
2. The current revision is well formed: every canonical fact has its secondary rows, every index row resolves, and every claim-index row resolves to one owner-scoped claim.
3. The revision includes the whole interrupted update or none of it.
4. `changesSince` from the last known-good token returns a contiguous delta or structured `TooOld`, never a partial delta.
5. Derived-index state reports a revision no newer than the graph revision; lag is visible.
6. Every persisted `s1_files_v1` content identity is compared against the working tree, and the resulting affected-file set matches the expected reindex set exactly. A file whose committed facts were lost must not appear current.
7. Reapplying the interrupted update converges to the same graph as applying it once, compared with [Conformance — Graph equality](../conformance.md#graph-equality).
8. The writer lock is acquirable by a new process without manual intervention and without relying on PID or timestamp heuristics.

Retained per case: point name, injected fault, occurrence ordinal, seed, generator tuple, observed tail loss, pre- and post-state snapshots, and the reopen transcript. A point that produces a defect becomes a named regression case.

### S1 project writer-lock evaluation (S-006)

Writer exclusion is the one S1 area where a wrong answer is silent: a second
project context that believes it holds the lock corrupts state slowly, and the
symptom appears later as inexplicable staleness. This section defines what the
spike must prove, using the `fs4` 1.1.0 pin in [Technology Candidates — S-006
writer-lock candidate pin](../technology-candidates.md#s-006-writer-lock-candidate-pin).
It tests the per-project mechanism; the separate per-user daemon lease and
cold-start race are tested by [F8 runtime](rust-foundation.md#f8-runtime-daemon-and-project-contexts).
It does not change the [single-writer contract](../storage.md#2-single-writer).

#### What the mechanism must provide

| Requirement | Why it cannot be weakened |
|---|---|
| Atomic acquisition | Two processes racing must produce exactly one winner, with no window where both proceed. |
| OS release on process death | A killed holder must release without any surviving process running cleanup code. |
| Ownership independent of metadata | PID, hostname, start time, and the presence of a lock file are diagnostics only. |
| Observable failure | An external contender must learn it lost; the daemon attaches observer/direct-only mode and never silently continues authoritatively. |
| Honest unsupported case | Where the platform or filesystem cannot guarantee exclusion, the adapter must fail closed rather than assume success. |

#### Cases

1. **Contention.** Start `N` project-context processes simultaneously; exactly one acquires. Repeat across process start orders and with the lock already held by an external process.
2. **Crash release.** `SIGKILL` the holder mid-transaction; a new process acquires without manual cleanup and finds the store at its last durable revision, using the `P2` expectations from [S1 durability and crash injection](#s1-durability-and-crash-injection-s-005).
3. **Stale metadata.** Leave a metadata record naming a dead holder; verify a new writer acquires the lock first and only then reclaims the record. Verify the reverse order is impossible.
4. **PID reuse irrelevance.** Recreate metadata whose recorded PID belongs to an unrelated live process; acquisition must be unaffected. A candidate that consults PIDs fails this case by construction.
5. **Lock-file deletion.** Delete the lock file while a writer holds it; the holder must not lose exclusion silently, and a second writer must not acquire on the strength of a recreated file.
6. **Observer fallback.** A losing daemon context attaches only when safe, reports observer/direct-only mode and `PROJECT_LEASE_UNAVAILABLE` for graph writes, observes revisions by comparison, and never buffers writes it cannot perform.
7. **Reacquisition after release.** Normal release, then immediate reacquisition by another process, repeated to expose leaked handles.
8. **Unsupported filesystem.** Exercise a filesystem without reliable advisory locking (network mounts are the realistic case) and verify the context refuses authoritative activation or remains direct-only, per [Architecture §11](../architecture.md#11-deployment-topologies). Silent success here is the worst possible outcome and is an automatic reject.
9. **Same-process double open.** Two handles inside one process must not both acquire; many OS advisory locks are per-process, not per-handle, so this is tested explicitly rather than assumed.
10. **Platform expansion.** Run cases 1–9 on Linux x86_64/glibc for the PoC. Repeat them on additional platforms only after the fully featured PoC is complete, recording per-platform semantics rather than generalizing from Linux.
11. **Coexistence with the index's own locking.** The pinned Tantivy `mmap` feature brings its own `fs4` at a different major version; verify the two do not interfere and record whether duplicate `fs4` majors coexist in one binary.

#### Measurements

- acquisition latency, uncontended and under contention by process count
- time from holder death to successful reacquisition
- false-acquisition count, which must be zero
- per-platform and per-filesystem support matrix

#### Pass conditions

- Exactly one writer at all times, on every platform and filesystem where the adapter reports support.
- Crash release requires no surviving process and no PID or timestamp heuristic.
- Metadata is never sufficient to claim, reclaim, or prove ownership.
- A losing context always reports observer/direct-only mode and
  `PROJECT_LEASE_UNAVAILABLE` observably.
- Every filesystem where exclusion cannot be guaranteed is reported as unsupported and fails closed.
- Any case where two writers proceed simultaneously rejects the candidate outright, regardless of its performance.

### S1 Measurements

- bulk load facts/second and bytes/second by batch size
- per-file replacement latency by owned node/edge count
- node lookup latency by id, name, and file
- outgoing and incoming traversal latency by degree
- snapshot-reader behavior during writes
- commit latency and reopen/recovery time
- observed commit tail loss per named fault point
- on-disk size by corpus and graph size

### S1 Pass conditions

- Store conformance behaviors applicable to the spike pass.
- Readers observe the old or new revision, never a mixture.
- Failed or interrupted writes never expose partial graph state. Every `(point, fault)` pair in [S1 durability and crash injection](#s1-durability-and-crash-injection-s-005) reaches its expected durable state, and recovery converges under [Conformance — Graph equality](../conformance.md#graph-equality). A commit that returned success is never lost to process termination. A tail of commits may be lost to OS or power loss provided nothing is torn and the affected files are detectably stale; the observed loss is measured and reported rather than compared to an invented threshold.
- Owner/file-scoped claim removal and reverse traversal use bounded indexed access rather than full graph scans; removing one producer cannot delete another producer's valid claim.
- Change history is atomic, normalized, contiguous or `TooOld`, durable across reopen, and compacted only at whole-revision boundaries.
- Stale-plan retries are bounded to two reprepare attempts per call; exhaustion commits nothing stale and schedules reconciliation.
- Durable reopen returns a well-formed committed revision.
- Writer contention is atomically enforced and reported; writes are never silently dropped. Crash release does not depend on PID/timestamp heuristics, and unsupported lock semantics fail closed or leave the context direct-only.
- No redb type or semantic is required by core contracts.

## 4. Experiment S2 — Tantivy lexical adapter

### S2 candidate pin

Use the S-007 `tantivy` 0.26.1 pin, source record, checksum, declared Rust 1.86 baseline, `mmap`-only feature set, and the explicitly disabled stemming/stop-word/compression features in [Technology Candidates — S-007 experiment pin](../technology-candidates.md#s-007-experiment-pin). The disposable S2 workspace MUST declare the exact version, commit `Cargo.lock`, and record its lockfile SHA-256, active features, and toolchain in each result using the [Experiment Result Template](template.md). Enabling `failpoints`, a compression feature, or any tokenizer feature produces a separately labelled run, never a baseline number. S2 MUST NOT infer Repin's eventual MSRV from this candidate's declared Rust version.

### S2 Questions

- Can Tantivy implement stable-key incremental removal and batched indexing?
- Can it support required filters and accurate evidence regions?
- Are advertised query modes deterministic and bounded?
- Can lag behind the graph be detected and repaired after interruption?

### S2 Prototype document

Index representative file and symbol documents with fields for:

- stable document key
- graph node id, where applicable
- root and relative path
- language
- artifact class
- node kind
- symbol and qualified names
- searchable text
- source range or offset mapping
- graph revision

### S2 experimental document schema (S-008)

This is the reviewable S2 schema. It implements the access paths the portable [Lexical port](../storage.md#5-lexical-port) requires and nothing more; it is not Repin's production index format, and it decides no naming, normalization, or public evidence-source contract. The portable `C-005` contract requires re-read verification; S2 stores regions **and** re-reads them so it can measure whether any bounded internal optimization could safely elide that work without weakening caller-visible evidence.

#### Document identity and granularity

Two document kinds share one schema, distinguished by a `docKind` field:

| Doc kind | One document per | Stable key |
|---|---|---|
| `file` | selected file version | `file:{RootId}:{path}` |
| `symbol` | graph node with searchable text | `symbol:{RootId}:{path}:{NodeId}` |

The stable key is the `DocKey` the port removes by. It is a single `STRING`-indexed, stored field so `remove(keys)` is an exact term delete, never a query. Keys embed `RootId` and path so removing every document for one file is a bounded prefix or term-set operation rather than a scan. A `symbol` key includes `NodeId` because two same-named symbols in one file must be independently removable.

Deletion in Tantivy is by term, so the schema guarantees every document has exactly one `docKey` term. S2 asserts that after `remove` plus commit, the deleted keys return no hits and the surviving keys are untouched.

#### Fields

| Field | Type and options | Purpose |
|---|---|---|
| `docKey` | `STRING` indexed, stored, single term | exact removal and hit-to-document mapping |
| `docKind` | `STRING` indexed, stored | separates file and symbol documents in filters |
| `rootId` | `STRING` indexed, stored | root filter and bounded removal |
| `path` | `STRING` indexed, stored | root-relative path filter and evidence resolution |
| `nodeId` | `STRING` indexed, stored, optional | graph validation of a hit; absent on `file` documents |
| `language` | `STRING` indexed, stored, optional | required metadata filter |
| `artifactClass` | `STRING` indexed, stored, optional | required metadata filter |
| `nodeKind` | `STRING` indexed, stored, optional | required metadata filter |
| `nameExact` | `STRING` indexed, stored | verbatim identifier as written |
| `qualifiedNameExact` | `STRING` indexed, stored, optional | verbatim qualified form |
| `nameSplit` | `TEXT` with the `code_split` tokenizer, positions indexed | subword identifier recall |
| `body` | `TEXT` with the `code_split` tokenizer, positions indexed | searchable content for phrase and term queries |
| `bodyOffsets` | stored bytes, not indexed | token-to-byte-offset mapping for `body` regions |
| `nameOffsets` | stored bytes, not indexed | byte range of the name within the file, for exact-field hits |
| `graphRevision` | stored, indexed as `STRING` | lag detection and stale-hit triage |
| `contentHash` | stored | tagged `Hash` of the indexed file version, used to detect a stale document |

`TEXT` fields with the `code_split` tokenizer record positions, because phrase queries are a required mode. `STRING` fields are untokenized single terms and carry **no positions**, so phrase queries target only `nameSplit` and `body`; exact fields support term, prefix, and regex modes only. All `STRING` fields are exact terms: a filter must never depend on tokenizer behavior.

#### Component encoding

Tantivy terms are UTF-8 text, while `RootId`, `NodeId`, and `Revision` are opaque bytes and a path is not guaranteed to be valid UTF-8 on every platform. S2 therefore encodes every opaque or possibly-non-UTF-8 component as lowercase hex before it enters a field or a `docKey`, and joins `docKey` components with `:`. Because hex never contains `:`, component boundaries are unambiguous and two different component tuples cannot collide into one key. `path` is indexed as the hex encoding of its exact bytes, with a separate stored human-readable form used for evidence output when those bytes are valid UTF-8. This is a spike encoding for term safety, not a decision about Repin's public identifier or path representation.

#### Tokenization

Code identifiers are not prose. `getUserName`, `user_name`, and `UserName` are related but distinct, and English stemming actively conflates unrelated symbols. S2 therefore indexes **both** an exact and a split representation of every name, and the schema states which field answers which query mode:

- `nameExact` and `qualifiedNameExact` use raw, untokenized, case-preserving terms. A search for `getUserName` finds that identifier verbatim, and case-sensitive distinctions survive.
- `nameSplit` and `body` use a spike-local `code_split` tokenizer. Its token stream is specified exactly, so the fixture oracle is reproducible:
  1. Split the input at every character that is neither a Unicode letter nor a digit, and additionally at every lowercase-to-uppercase and letter-to-digit boundary.
  2. Emit the **whole original token first** at position `p`, unmodified, with its exact source byte offsets.
  3. Emit each part in source order at positions `p+1, p+2, …`, each with its own exact source byte offsets, lowercased using Unicode simple case folding. Case folding never alters recorded offsets, which always refer to original bytes.
  4. Advance `p` past the last emitted part before the next original token, so parts of different identifiers are never positionally adjacent. When a token has exactly one part identical to the original, the duplicate part is **not** emitted: a single-word identifier produces one token, not two identical ones at the same span.
  5. Stemming and stop words stay disabled.

  So `getUserName` yields, in order, `getUserName` unmodified, then `get`, `user`, `name`.
- Because parts carry consecutive positions, the phrase `user name` matches inside `getUserName`. That follows from the stated position rule rather than being assumed.
- Regex targets exact fields only, since regex over split tokens would match fragments a user never wrote. Exact fields have no token offsets, so a regex or term hit's region comes from stored `nameOffsets` for a name match and from re-reading the file otherwise. Index-derived regex spans over `body` are out of scope for S2.

The cost is index size and one deliberate ranking hazard: the same document matches through two fields, so a naive score sums duplicate evidence. S2 measures the size overhead and reports how it ranks exact versus split matches; it does not silently invent a ranking formula and call it a result.

#### Query analysis and mode-to-field mapping

Index-time analysis is only half the contract; a query analyzed differently from the document cannot match it. S2 fixes the mapping for each `LexicalQuery.mode` in the [Lexical port](../storage.md#5-lexical-port):

| Mode | Fields searched | Query-side analysis |
|---|---|---|
| `terms` | `nameExact`, `qualifiedNameExact`, `nameSplit`, `body` | exact fields receive the raw query text as one term; split fields receive the query through the identical `code_split` pipeline, so query and document agree |
| `phrase` | `nameSplit`, `body` only | `code_split`, with the resulting token sequence used in order; exact fields carry no positions and are not searched |
| `prefix` | `nameExact`, `qualifiedNameExact`, `nameSplit`, `body` | the trailing token is treated as a prefix; on exact fields the whole raw query is the prefix, on split fields the query is split and only its final part is a prefix |
| `regex` | `nameExact`, `qualifiedNameExact` only | pattern applied to raw terms with no analysis; regex over split tokens would match fragments a user never wrote |

A mode searches every field listed for it and unions the results by document key. Because `code_split` emits both the unmodified original token and lowercased parts, a query for `getUserName` matches the original token exactly while a query for `username` matches through the folded parts. The one asymmetry is deliberate and recorded: exact-field matching is case-sensitive, split-field matching is case-insensitive, so the same query can match one field and not the other. S2 reports that rather than hiding it behind a merged score.

#### Filter support

All filter fields are indexed as exact `STRING` terms, so S2 advertises **exact-value filters only**. [Retrieval](../retrieval.md) types `paths` and `exclude` as `PathPattern[]`; S2's `LexicalCapabilities.filters` therefore lists only the exact-match filter kinds it truly supports, and a non-exact path pattern is rejected as an unsupported capability rather than silently reinterpreted as an exact path. Pattern-to-index translation is a real design question, but it is [Retrieval](../retrieval.md)'s and the coordinator's to answer with evidence, not something the spike may quietly assume.

This tokenizer is an S2 hypothesis. It is not a language-aware analyzer, it makes no claim about non-Latin identifiers beyond the fixture's Unicode cases, its case folding is simple rather than full, and it does not decide Repin's eventual analysis policy.

#### Evidence regions

A hit must produce an accurate root-relative path and source region. S2 obtains regions two ways and compares them:

1. **From the index.** For a `body` or `nameSplit` hit, token positions plus stored offsets produce a byte range. `nameOffsets` gives the span of the whole name, so it is used only when the match covers the whole term. A prefix or regex match that covers part of an exact term does **not** get a whole-name range: its exact span is located by re-reading, because reporting the enclosing identifier where a substring matched would violate the fixture's exact-span requirement.
2. **From the working tree.** The file is re-read, its `contentHash` compared to the stored one, and the region located in current bytes.

When the hashes agree, both paths must yield identical byte ranges; a divergence is an index defect. When they disagree, the document is stale, and the case records whether the adapter suppressed the hit, revalidated it, or wrongly returned an index-derived region for content that no longer exists. That comparison measures the cost and limits of the already-defined `C-005` verification contract.

#### Revision and staleness

`graphRevision` and `contentHash` are stored per document so that reopen can classify each hit as current, lagging, or stale without consulting the store first. This is deliberately redundant with authoritative state: the graph store remains authoritative, and a lexical document never overrides it. A hit whose `nodeId` no longer exists in the graph is dropped during normalization regardless of what the index says, per [Storage §7](../storage.md#7-consistency-between-indexes).

#### Schema versioning

The schema records a spike-local `s2SchemaVersion`. Changing any field, option, or tokenizer rule increments it and invalidates affected S2 evidence, exactly as an S1 key-encoding change does. Tantivy cannot reinterpret an existing index under a changed schema, so a version change means rebuild, which is itself one of the S2 cases.

### S2 lexical evidence fixture (S-009)

The fixture is the oracle for every S2 correctness case. It follows the generator rules in [Initial Fixture and Corpus Manifest §6](fixtures.md#6-deterministic-graph-shape-generators) — seeded, canonical serialization, fixed-width ordinals — and is generated rather than hand-maintained so expected offsets cannot drift from content.

Every case declares, for each query, the exact expected document keys **and** the expected byte ranges, in ascending order. A case that only asserts "some hit was returned" is not acceptable evidence.

| Family | Content | What it must prove |
|---|---|---|
| `L-ASCII` | plain ASCII identifiers and prose | baseline exactness of ranges and keys |
| `L-UNICODE` | non-ASCII identifiers, combining marks, emoji, right-to-left text | byte ranges stay on character boundaries; no offset drift from multi-byte sequences |
| `L-CRLF` | identical content in LF and CRLF | line endings shift byte offsets without changing which symbols match |
| `L-CASE` | `getUserName`, `getusername`, `GETUSERNAME`, `get_user_name` in one file | exact field preserves case; split field provides subword recall; the two are distinguishable |
| `L-PHRASE` | adjacent, separated, and reordered token sequences | phrase queries respect position and do not match reordered text |
| `L-PREFIX` | shared prefixes of increasing length, plus a prefix that is also a whole token | prefix mode bounds its expansion and does not silently truncate matches |
| `L-REGEX` | text with regex metacharacters, and anchored/unanchored targets | regex mode returns exact spans over exact fields; metacharacters in content are not interpreted |
| `L-OVERLAP` | one token matching several queries, and nested/overlapping match candidates | overlapping matches are reported deterministically with a stated policy, not arbitrary order |
| `L-LONG` | a very long single line and a very large file | region computation does not degrade to whole-file scanning; resource guards hold |
| `L-DUP` | the same identifier repeated many times in one file | every occurrence has a distinct region; occurrence count is reported, not collapsed |
| `L-STALE` | a file version indexed, then edited on disk | hash mismatch is detected and an index-derived region is never presented as current |
| `L-DELETE` | multi-symbol files where one symbol, then the whole file, is removed | term deletion is exact and bounded; siblings survive |

Range expectations are recorded as byte offsets, with character offsets alongside for any case containing non-ASCII content, so a byte-versus-character defect cannot pass by matching only one representation. The fixture also states, per query, whether it is expected to return zero hits — negative cases are required, since a permissive tokenizer fails by over-matching, not by under-matching.

Two policies the oracle depends on are fixed here, so `L-OVERLAP` and `L-PREFIX` have exact expectations rather than requiring a policy that does not exist:

- **Overlap policy.** All matches are retained, never merged. Within one document they are ordered by ascending start offset, then descending end offset, then token position, then field name, so ordering is total and a nested match deterministically follows its enclosing match. A span reached through both the exact and the split field is reported twice, because collapsing the two would hide the dual-field ranking hazard this schema deliberately introduces. The portable `LexicalHit.regions` carries only ranges, so the field that produced each region is **test-only fixture metadata** obtained from the spike's own inspection surface; S2 does not add field identity to the port.
- **Prefix expansion bound.** Prefix and regex term expansion is capped at a configured maximum term count, recorded per run. Reaching the cap is never silent. `Lexical.query` returns hits without a status envelope, so the adapter surfaces the cap through its own bounded-expansion signal, and the coordinator renders it as `Truncation` with `truncated: true` and `reason: limit` in the caller-facing [result](../results.md#truncation). The fixture asserts the caller-facing envelope, not just the raw hit list, since silent truncation is exactly the failure being tested.

### S2 lexical repair alternatives (S-010)

When the graph commits and the lexical index does not, S2 must repair rather than trust stale state. Three protocols are candidates; S2 implements all three and compares them, because the cheapest correct one is not obvious in advance.

| Protocol | How it repairs | Cost | Failure mode to test |
|---|---|---|---|
| **R1 pending-work journal** | the store durably records per-index pending work at commit; recovery replays it | extra write per commit; journal must itself be crash-safe | a journal entry whose work was partly applied; journal and index disagreeing |
| **R2 revision-diff reconstruction** | recovery reads `changesSince(lexicalRevision)` and rebuilds only affected documents | no per-commit cost; depends on durable history | history compacted past the lexical revision, returning `TooOld`; **no** recorded lexical revision at all, which also forces R3 |
| **R3 full rebuild** | discard the index and rebuild from graph plus working tree | always correct, always most expensive | rebuild interrupted by a second failure |

All three must satisfy the same invariants: the graph is never mutated to match the index, repair is idempotent under repeated interruption, and the repaired index is **lexically equivalent** to a fresh rebuild.

Lexical equivalence needs its own oracle, because [Graph equality](../conformance.md#graph-equality) deliberately ignores adapter-internal index state and therefore proves nothing about an index. Two lexical indexes are equivalent when, across the entire [S2 evidence fixture](#s2-lexical-evidence-fixture-s-009) query corpus, both return the same document keys, the same regions in bytes and characters, the same relative ordering or the same declared tie-break, the same filter results, the same truncation signals, and the same empty results for every negative case. Graph equality is used separately, and only to confirm that repair did not mutate authoritative state.

The protocols may also compose, and S2 tests composition as well as each protocol alone. Which protocol becomes the normal path, and what the fallback order is, is **not decided here**: [Technology Candidates §7](../technology-candidates.md#7-cross-index-model-to-validate) leaves the journal-versus-reconstruction question open, and plan finalization settles it from S2 and S4 evidence. The decisive measured question is what R2 costs once history has been compacted — `TooOld` forces R3, so R2's practical value depends on the retention defaults that P-012 deliberately left to evidence.

S2 records, for each protocol: repair time by change volume, work-record size, whether repair after interruption reaches lexical equivalence with a fresh rebuild, and the smallest change volume at which full rebuild becomes cheaper than incremental repair. That crossover is the number plan finalization needs; it is measured, not assumed.

### S2 Cases

1. Build an initial index, commit, reopen, and repeat identical queries.
2. Delete documents by stable key and add replacements in batches; verify sibling documents in the same file survive.
3. Filter by every required metadata category, singly and in combination.
4. Exercise term, phrase, prefix, and regex modes against both exact and split fields; advertise only modes that meet the contract.
5. Verify every hit maps to an accurate root-relative path and source region, comparing the index-derived and re-read regions from the [S2 document schema](#s2-experimental-document-schema-s-008).
6. Run every family in the [S2 evidence fixture](#s2-lexical-evidence-fixture-s-009), including its negative cases, and compare exact keys and byte and character ranges.
7. Simulate interruption before and after Tantivy commit.
8. Open with graph revision ahead of lexical revision; detect the lag and repair it with each protocol in [S2 repair alternatives](#s2-lexical-repair-alternatives-s-010).
9. Force `TooOld` by compacting history past the lexical revision; verify the fallback to full rebuild.
10. Interrupt each repair protocol partway and repeat it; verify idempotence.
11. Return a stale hit whose graph node was deleted; validate that the engine drops it.
12. Rebuild the complete index from authoritative graph and working-tree inputs.
13. Compare repeated query ordering for identical state.

### S2 Measurements

- initial indexing throughput by batch size
- incremental delete/add/commit latency
- filtered query latency and candidate counts
- phrase, prefix, and regex query cost
- exact-versus-split field index size overhead
- reopen, lag-check, repair, and full-rebuild time
- per-protocol repair cost by change volume, and the crossover volume where full rebuild is cheaper
- index size and merge behavior across update sequences

### S2 Pass conditions

- Lexical conformance behaviors applicable to the spike pass.
- Incremental replacement does not return deleted documents after commit, and removing one document never removes a sibling.
- Every returned hit has an accurate evidence region: index-derived and re-read ranges agree whenever the content hash matches, in bytes and in characters.
- Every fixture family passes, including its negative cases; over-matching is a failure.
- Filters are correct and independent of tokenizer behavior; unsupported modes are honestly excluded from capabilities.
- Identical committed state and query produce stable ordering or a stable adapter-level tie-break.
- Revision mismatch is observable and recoverable without changing authoritative graph state; at least one repair protocol reaches lexical equivalence with a fresh rebuild and remains idempotent under repeated interruption.
- A stale hit cannot escape result normalization as a dangling entity.

## 5. Experiment S3 — USearch vector adapter

**Status: deferred (S-012).** The decision is recorded in [Technology Candidates — S-012 USearch deferral](../technology-candidates.md#s-012-usearch-deferral): S3 stays fully specified but does not run in Stage 2, it reopens when semantic or hybrid retrieval enters an implementation milestone, and at reopen it evaluates USearch alongside at least one candidate from the [S-013 shortlist](../technology-candidates.md#s-013-vector-candidate-shortlist) against the identical `Vector` contract. Deferral is an explicit outcome, not a failure of deterministic planning, and not permission to treat any vector candidate as accepted.

Until then, no shipped capability, benchmark claim, or release artifact may depend on a vector index, and Stage 2 exit is evaluated without S3 evidence.

### S3 Questions

- Does an acknowledged deletion prevent an entry from being returned, including after reopen?
- Are persistence and reopen behavior adequate for a rebuildable index?
- Can metadata filters be applied correctly and efficiently?
- If post-filtering is required, what over-fetch bound preserves useful recall?
- What native build, linking, and distribution constraints does the Rust binding introduce?

### S3 Cases

1. Insert deterministic test vectors and verify distance ordering.
2. Upsert an existing key and verify the old vector cannot surface.
3. Delete entries, reopen, and query near their former vectors.
4. Enforce dimension and metric consistency.
5. Filter by root, language, artifact class, and node kind, individually and together.
6. If filtering is external, measure adversarial distributions where valid matches are rare.
7. Run concurrent searches while applying bounded update batches.
8. Terminate during update/persist operations; reopen or rebuild cleanly.
9. Delete all chunks for one graph node and verify none return.
10. Build release artifacts on every proposed target platform.
11. Run every case against at least one [shortlist alternative](../technology-candidates.md#s-013-vector-candidate-shortlist), so rejection and replanning stay distinguishable.

### S3 Measurements

- build and incremental upsert throughput
- search latency and recall by corpus size and search parameters
- deletion and node-wide removal latency
- filtered-search candidate amplification
- memory and disk footprint
- reopen and rebuild time
- release binary size and build complexity by platform

### S3 Pass conditions

- Vector conformance behaviors applicable to the spike pass.
- Acknowledged deleted and superseded entries never appear in returned results, regardless of whether the implementation uses internal tombstones.
- Dimension and metric mismatches fail explicitly.
- Metadata filtering is correct. Any post-filter strategy is bounded and has honest truncation/coverage behavior.
- Corrupt or interrupted state can be discarded and rebuilt without affecting deterministic capabilities.
- Native distribution constraints are acceptable for the finalized support matrix.
- Absence or failure affects semantic retrieval only.
- At least one alternative candidate was evaluated against the same cases, so an acceptance is a comparison rather than a default.

## 6. Experiment S4 — Combined revision and recovery protocol

S4 runs with the `Vector` adapter absent while S3 is deferred. Every vector row below is then exercised through a null/absent vector adapter: the requirement that deterministic revisions stay current and that lag is visible must hold when there is no vector index at all, which is the configuration Stage 2 actually ships evidence for.

### S4 Questions

- What exact protocol keeps the graph authoritative while derived indexes commit independently?
- Is a pending-work journal needed, or is revision-based reconstruction sufficient?
- Can every interruption point converge through repair or rebuild?

### S4 Failure matrix

Test termination at least at these points:

| Point | Expected state after reopen |
|---|---|
| before graph commit | old graph and old derived indexes remain valid |
| after graph commit, before lexical update | new graph authoritative; lexical lag visible and repairable |
| during lexical update, before lexical commit | new graph authoritative; lexical old or recoverable, never trusted as current |
| after lexical commit, before completion marker | mismatch recognized; idempotent repair or verification succeeds |
| while vector work is pending | deterministic revisions remain current; vector lag and pending count visible |
| after node deletion, before derived deletion | stale derived hits are dropped through graph validation |

Repeat each case across process restart and repeated recovery to prove idempotence.

### S4 failure oracle (S-011)

The matrix above names states in prose. This oracle makes each one checkable: for every interruption point it states the exact expected graph revision, per-index acknowledged revision, pending-work state, and query behavior. A case passes only if all four match. "Recoverable" and "visible" are not acceptable assertions on their own.

Notation, using the revision before the interrupted update as `Rn` and the update's revision as `Rn+1`:

| Symbol | Meaning |
|---|---|
| `G` | authoritative graph revision reported by the store |
| `Lack` | lexical acknowledged revision |
| `Vack` | vector acknowledged revision, or `absent` when no vector adapter is configured |
| `Wlex`, `Wvec` | pending derived work for that index, by target revision |
| `partial` | a result explicitly marked as partial coverage with a warning |

When no vector adapter is configured — the Stage 2 configuration while S3 is deferred — `Vack` is `absent` and `Wvec` is always `none`. No pending vector work may exist for an index that does not exist. Rows below give the vector columns for the enabled case; with the adapter absent, read `Wvec` as `none` throughout.

| Interruption point | `G` | `Lack` | `Wlex` | `Vack` | `Wvec` | Query behavior |
|---|---|---|---|---|---|---|
| `T1` before graph commit | `Rn` | `Rn` | none | `Rn` or `absent` | none | all channels current; no warning |
| `T2` after graph commit, before lexical update starts | `Rn+1` | `Rn` | target `Rn+1` | `Rn` or `absent` | target `Rn+1` when the vector adapter is enabled, otherwise `none` | graph/direct current; lexical bypassed as known-lagging; substituted direct scan; warning present; `partial` only if fallback coverage is incomplete |
| `T3` during lexical update, before lexical commit | `Rn+1` | `Rn` | target `Rn+1` | unchanged | unchanged | identical to `T2`; a partially written lexical segment must not raise `Lack` |
| `T4` after lexical commit, before the completion marker | `Rn+1` | `Rn` | target `Rn+1` | unchanged | unchanged | as `T2`; repair must be idempotent and must not double-apply — reindexing already-current documents is required to be a no-op |
| `T5` after lexical acknowledgement | `Rn+1` | `Rn+1` | none | unchanged | unchanged | lexical current; no lexical warning; vector may still lag |
| `T6` while vector work is pending | `Rn+1` | `Rn+1` | none | `Rn` | target `Rn+1` | deterministic channels fully current, status `ok`/`not_found`, no deterministic warning; vector lag and pending count visible in `IndexStatus.semanticState`/`semanticPending`. Semantic hits from the lagging index may still be merged and ranked normally, per [Optional Intelligence](../intelligence.md), but a hit whose graph node no longer exists is dropped. Semantic lag alone never downgrades a deterministic result |
| `T7` after node deletion, before derived deletion | `Rn+1` | `Rn` | target `Rn+1` | unchanged | unchanged | lexical is known-lagging, so it is bypassed and a current direct scan substitutes, exactly as `T2`; the deleted node cannot reach a caller. Stale-hit dropping is tested separately as an internal defense by querying the lagging index directly |
| `T8` during repair itself | `Rn+1` | `Rn` | target `Rn+1`, unchanged | unchanged | unchanged | repair restarts cleanly; no acknowledgement advanced by an interrupted repair |
| `T9a` lexical index files deleted from the `T5` current state, state record intact | `Rn+1` | `Rn+1`, preserved | `full-rebuild(target Rn+1)` until acknowledged | unchanged | unchanged | the durable acknowledgement is unchanged, but the index reports `building` rather than `current`, a rebuild is enqueued, and the rebuilt index reaches lexical equivalence. A preserved acknowledgement must never make a missing index look queryable |
| `T9b` lexical index files **and** its state record deleted | `Rn+1` | absent | `full-rebuild(target Rn+1)` | unchanged | unchanged | absent state is treated as never-acknowledged, never as current; full rebuild restores it without touching `G`. R2 cannot apply with no recorded revision, so this case exercises the R3 fallback |

Both `T9` rows start from the `T5` state, where lexical was current, because that is the only starting point where "deleted index still claims to be current" is a real hazard. The same deletion applied to a `T2`/`T4` lagging state is also run, and must produce the same `building`-or-absent classification.

`T9a` needs one status value the current contract lacks: a lexical index whose state record says `current` but whose files are gone is neither `current`, `bypassed_lagging`, `disabled`, nor `failing`. S2 reports it as `building`, matching `graphState`'s existing `building` value in [Results and Evidence — Freshness](../results.md#freshness). Whether `building` is added to the portable `lexicalState` and `semanticState` unions is a contract question this experiment feeds rather than settles; until then the spike records it as a deviation.

The `T7` split matters: the storage contract in [Storage §7](../storage.md#7-consistency-between-indexes) says a *known*-lagging lexical index is not queried at all, so a user-facing query at `T7` must never have the opportunity to return the deleted node. Graph validation dropping a stale hit is the second line of defense for unknown or accidentally queried index state, and it is exercised deliberately rather than relied on as the primary behavior.

The `T9a`/`T9b` split matters for the same reason: deleting index files out of band does not erase the acknowledgement record held in the authoritative store, so the two cases have genuinely different expected quadruples. Conflating them would make the "exact quadruple" claim unverifiable.

Three invariants apply at every point and are asserted separately from the table:

1. `G` never decreases, and no derived index ever reports an acknowledged revision **newer** than `G`. A derived index ahead of the graph is a defect, not lag.
2. `Lack` and `Vack` advance only on explicit acknowledgement. An interrupted update, an interrupted repair, or a partially written segment never advances them.
3. After repair or rebuild completes, the authoritative graph compares equal to a fresh rebuild from the same final working tree under [Conformance — Graph equality](../conformance.md#graph-equality), and the repaired lexical index reaches lexical equivalence with the fresh one as defined in [S2 repair alternatives](#s2-lexical-repair-alternatives-s-010).

Each point is exercised across process restart and repeated recovery, and every case records the observed quadruple `(G, Lack, Wlex, Vack/Wvec)` rather than a pass/fail flag alone, so an unexpected-but-passing state is still visible in the evidence.

### S4 Cases

1. Apply create, modify, delete, rename, and bulk-change batches.
2. Inject termination at every point `T1`–`T9b` in the [S4 failure oracle](#s4-failure-oracle-s-011) and assert its full expected quadruple.
3. Reopen, report per-index revisions, repair, and verify convergence.
4. Interrupt the repair itself and repeat.
5. Corrupt or delete each derived index independently and rebuild it.
6. Verify direct retrieval throughout graph or derived-index unavailability.
7. Compare the repaired state with a fresh rebuild from the same final working tree.
8. Run the whole matrix with no vector adapter configured, which is the Stage 2 configuration while S3 is deferred.

### S4 Pass conditions

- The graph is never rolled back or mutated to match a derived index, and no derived index ever reports a revision newer than the graph.
- Every point in the [S4 failure oracle](#s4-failure-oracle-s-011) reaches its exact expected revisions, pending-work state, and query behavior.
- Every lag state is visible through status and freshness metadata.
- Recovery is idempotent and converges to a fresh rebuild.
- Dangling lexical or vector hits are never returned.
- Vector work never blocks a deterministic graph revision, including when no vector adapter exists.
- After context unload, deleting the complete `.repin` directory remains a
  sufficient final recovery action; active deletion fails the context closed
  and requires a new activation cycle.

## 7. Experiment outputs

Store experiment artifacts under a dedicated, clearly non-production area when experimentation begins. Each completed experiment produces:

```text
experiment id and date
question and candidate version
source/fixture revision
method and commands
raw results
observed failures and retained reproduction seeds
contract deviations
conclusion: accept | reject | defer | revise experiment
```

Do not turn a spike conclusion directly into production code. During plan finalization:

The per-family result and recommendation ledger is [Experiment Results](results/index.md). Reports with `pending` or `deferred` status are not candidate acceptance.

1. Review the evidence against the port contract.
2. Accept, reject, or defer each candidate.
3. Record accepted decisions under `docs/decisions/`.
4. Finalize adapter boundaries, state protocol, support matrix, and implementation milestones.
5. Start production implementation only after all blocking decisions are resolved.
