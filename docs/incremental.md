# Incremental Updates

How the graph stays current. This is the subsystem where correctness is hardest to preserve and easiest to lose quietly, so most of this document is invariants rather than mechanism.

The governing property is stated first because everything else exists to serve it.

## 1. The convergence invariant

```text
fresh_index(final_state)
  ==
apply_changes(fresh_index(initial_state), change_sequence)
```

Applying any sequence of changes incrementally MUST produce a graph identical to indexing the final state from scratch: same nodes, same edges, same resolution outcomes, same unresolved set. Revisions, timestamps, and internal ordering are excluded from the comparison; everything else is compared exactly.

This is not an aspiration or a quality target. It is the property that makes an incremental engine trustworthy, and the characteristic failure of such engines is drifting away from it slowly enough that nobody notices until answers are wrong. A divergence is a correctness bug of the highest severity, never an acceptable approximation.

Two corollaries:

- **Order independence.** Any interleaving of the same changes converges to the same graph.
- **Restartability.** Interrupting and resuming an update sequence converges. A crash mid-update cannot leave a graph that differs from one built cleanly.

[Conformance](conformance.md) specifies the replay harness that asserts this continuously.

## 2. Change model

```text
FileChange
  = Create { root, path, origin, content? }
  | Modify { root, path, origin, content? }
  | Delete { root, path, origin }
  | Rename { root, from, to, origin, content? }
```

```text
ChangeOrigin = watcher | host | cli | scan | vcs
```

Two separate concerns, deliberately not merged into one field:

- **`content`** is an optional optimization. Supplied, the engine skips reading the file; absent, it reads from disk. Meaningless on `Delete`, and therefore absent from that variant rather than present-and-ignored.
- **`origin`** identifies the reporter. It affects deduplication and diagnostics **only**, and MUST NOT influence extraction. Two changes with identical content and different origins produce identical facts.

A single field cannot serve both roles, because deduplicating a host-reported edit against the watcher event for the same write requires knowing the reporter *and* the content simultaneously.

## 3. The update primitive

```text
updateFiles(changes: FileChange[]) -> UpdateSummary
```

This is the real primitive. Everything else is a producer feeding it:

```text
   watcher ─┐
   host ────┤
   cli ─────┼──> coalesce ──> updateFiles ──> transaction ──> revision
   scan ────┤
   vcs ─────┘
```

Consequences worth making explicit:

- The watcher is **one producer among several**, not the mechanism. An engine designed watcher-first cannot accept a direct notification, which costs an entire debounce interval on every host edit.
- A host that edits a file can report it immediately and have it queryable in milliseconds.
- Batch operations, initial scans, and version-control-derived change sets all enter through the same path, so there is one code path to make correct.

```text
pause()  -> void
resume() -> UpdateSummary
```

`pause` suspends processing and accumulates. `resume` evaluates everything accumulated as one batch, which may escalate ([§7](#7-backpressure)). Used around known-bulk operations. It is an optimization: correctness MUST NOT depend on a producer announcing a bulk operation, because most will not.

## 4. Deduplication

Key: `(root, path, contentHash.algorithm, contentHash.digest)`.

`contentHash` uses the tagged `Hash` contract in [Results and Evidence §2](results.md#2-evidence). A change whose path, hash algorithm, and digest match the most recently committed state for that path is **already applied** and is dropped, regardless of origin. Different algorithms are never equal merely because their digest bytes match.

Content-based rather than time-window-based, deliberately. A timing window is a guess about watcher latency, and it is wrong under load — exactly when duplicate suppression matters most. Content identity is exact and requires no tuning.

This is what makes host notification safe to combine with an active watcher: the host reports the write, the watcher reports it again moments later, and the second one costs a hash comparison.

## 5. Transactions

Clients MUST NOT observe a partially updated graph.

```text
PREPARE OUTSIDE WRITE TRANSACTION
  read, hash, parse, and extract replacement facts
  compute the affected resolution region

BEGIN AUTHORITATIVE STORE TRANSACTION
  remove facts owned by the previous version of each changed file
  insert replacement facts
  resolve affected references
  promote newly-resolvable unresolved references
  demote references whose targets disappeared
  record skips, diagnostics, and derived-index intent
  increment revision
COMMIT

RECONCILE DERIVED INDEXES
  update lexical index and acknowledge its revision
  enqueue semantic work
```

The prepared input is a `FileSnapshot` with the named root, root-relative
path, source (`host_supplied` or `filesystem`), exact bytes, tagged content
hash, and observed file identity/size metadata when read from the filesystem.
For a filesystem read, the engine opens through the root-confined mechanism,
reads from that handle, and validates the handle's identity/size after the
read. A change during the read is an unstable snapshot, not a valid partial
file. For host-supplied content, the supplied bytes and tagged hash are the
input for that call; root and selection checks still run, and optional host
identity metadata is an additional guard. The snapshot is bound to the
prepared graph revision and is revalidated before commit. This is the
prepare/revalidate contract exercised by [Rust Foundation F3](experiments/rust-foundation.md#f3-preparation-snapshot-and-revalidation-state-machine).

Rules:

- A query observes either the previous revision or the new one. Never a mixture, never an intermediate state.
- Failure before or during the authoritative store transaction rolls back entirely. A failed graph update leaves the previous revision intact and queryable.
- A revision is created only on successful authoritative commit. Revision numbers correspond one-to-one with committed graph states.
- Reads, parsing, extraction, and other fallible expensive preparation SHOULD occur before opening the write transaction. The plan records its base graph revision and each input's content identity. Commit conditionally verifies both. On mismatch, the operation reparses/replans at most **two automatic attempts** within the original cancellation/deadline budget, coalescing newly observed changes into each attempt. The retry counter applies to the whole API call, not independently per file. If the second reprepare is stale, the engine commits none of that stale plan, retains/coalesces the affected roots for reconciliation scan, and returns an explicit retryable conflict outcome. It does not spin until the tree becomes quiet. This keeps writer hold time bounded without committing stale resolution decisions or facts for bytes that are no longer current.
- A separate lexical port cannot join the authoritative transaction. Its post-commit failure leaves the new graph revision valid and produces an observable, repairable lexical lag—not a rollback of graph facts. See [Storage §7](storage.md#7-consistency-between-indexes).
- Exactly one authoritative write transaction is in flight at a time ([Storage §2](storage.md#2-single-writer)). Concurrent update requests coalesce into the next batch rather than queueing transactions, which bounds latency under churn instead of letting it grow without limit.
- Semantic-index mutations are serialized through the same writer coordinator but use their own physical index transaction. Deterministic graph commits take priority and never wait for embedding work.

## 6. Invalidation

Not every change requires a global re-index, and not every change is local. For each changed file:

1. remove or invalidate facts owned by the previous version
2. parse the new content
3. update locally-owned entities
4. identify affected incoming and outgoing references
5. re-resolve only the affected dependency region
6. update affected indexes
7. commit

### Blast radius classification

```text
local           facts inside one file; nothing outside re-resolves
dependency      exported surface changed; importers re-resolve
global          resolution rules themselves changed; broad re-resolution
```

Changes that are usually `global`:

- package and workspace manifests
- compiler or language configuration affecting module resolution
- dependency lock files
- barrel and re-export aggregation files
- path mapping and alias configuration
- the engine's own selection or classification rules

Classification is **conservative**: when unsure, escalate. An over-escalated update is slow; an under-escalated one is wrong, and wrong in a way that persists until something else happens to invalidate the affected region.

## 7. Backpressure

Ordinary developer actions produce thousands of events in under a second: branch switches, dependency installs, code generation, rebases, bulk formatting. An unbounded queue in front of `updateFiles` will be overwhelmed by routine work.

- The pending queue is **bounded**.
- On overflow, the engine **discards the queue and escalates to a scan** of the affected roots. It MUST NOT process a partial event stream — a partial stream produces a graph that diverges from the filesystem with no indication that it has.
- Above a threshold count in one batch, escalate from per-file invalidation to bulk re-resolution, because per-file work stops being cheaper than rebuilding the affected region.
- Escalation is always safe: a scan converges by definition ([§1](#1-the-convergence-invariant)).

The design stance is that dropping to a scan is a normal, healthy response to churn, not an error path.

## 8. Unresolved references

The mechanism that makes resolution convergent. Without it, an incremental engine cannot be correct.

The problem: file A references a symbol that does not exist yet. Later, file B defines it. Nothing in a purely backward-looking invalidation scheme revisits A, so the reference stays broken forever — and it stays broken silently, degrading the graph across a long session.

```text
UnresolvedRef
  from:       NodeId
  seeking:    Text          // normalized name being sought
  scopeHint?: Text          // module specifier or scope, when the reference had one
  edgeKind:   EdgeKind
  provenance: Provenance
```

Stored with a **reverse index keyed by `seeking`**, which is what makes the promotion step cheap enough to run on every commit.

On every commit, after inserting definitions:

1. collect newly-defined qualified names in this batch
2. look that set up in the reverse index
3. promote each match to a real edge
4. delete the promoted entries

Symmetrically, when definitions are removed, demote edges that pointed at them back to unresolved references, retaining `seeking`.

This yields order independence: definitions and references may arrive in any order and converge to the same graph.

Unresolved references are **queryable**, which makes them useful beyond correctness:

- "What references does the graph not understand?" is a direct measure of extractor coverage.
- The unresolved ratio is an input to `coverage` in result freshness ([Results and Evidence](results.md)), so callers can tell whether an absent answer means anything.

## 9. Version control integration

Version control is the dominant source of high-fan-out change in real use. Treating it as first-class is cheaper than discovering it through the watcher.

**Startup detection.** The store records the VCS revision identifier and dirty-file set observed at the last commit. On open, ask the VCS what changed rather than crawling and hashing everything — diffing two revisions is dramatically cheaper than hashing an entire tree.

Fall back to a full scan when: the directory is not under version control, the VCS is unavailable, the recorded revision is unreachable (history rewrite, shallow clone), or the recorded state is inconsistent.

**Branch operations.** A branch switch or rebase is a bulk change and escalates ([§7](#7-backpressure)). It MUST NOT be processed as thousands of individual updates.

**Scope.** The engine indexes the **working tree**, not history. VCS internals are never indexed, though a branch pointer may be observed to detect switches. History as graph content is explicitly out of scope; it is a different product with different storage characteristics.

## 10. Revisions

```text
Revision
```

Opaque to clients, comparable for equality only ([Results and Evidence §1](results.md#1-envelope)). Internally monotonic. Never reused, including across compaction and migration.

```text
UpdateSummary
  revision:      Revision
  files:         FileChange[]
  addedNodes:    NodeId[]
  changedNodes:  NodeId[]
  removedNodes:  NodeId[]
  addedEdges:    EdgeId[]
  removedEdges:  EdgeId[]
  resolved:      Count       // unresolved refs promoted
  unresolved:    Count       // refs demoted or newly unresolved
  skipped:       Skip[]
  escalation?:   Escalation  // when the update was escalated, and why
```

The added/changed/removed split is only meaningful because IDs are position-independent ([Graph Model §4](graph-model.md#4-identity)). With position in the ID, every node in an edited file appears as changed and the report conveys nothing.

## 11. Change history

```text
changesSince(revision) -> ChangesResult

ChangesResult
  = Ok      { from: Revision, to: Revision, updates: UpdateSummary[] }
  | TooOld  {
      requested:       Revision
      oldestAvailable: Revision
      current:         Revision
      reason:          compacted | unknown_revision
    }
```

Each authoritative commit stores one immutable, normalized `UpdateSummary` under the resulting revision in the **same transaction** as graph facts and the current-revision record. Lists and maps use canonical ordering, duplicate IDs are eliminated, and summaries contain IDs/file changes rather than copied node bodies. A no-op produces no graph revision and therefore no history entry. Reopen observes either both graph revision and summary or neither.

Retention is bounded by **both** whole-entry count and age, whichever binds first. Experiments compare candidate policies at 1,000/10,000/100,000 entries and 7/30/90 days before plan finalization chooses numeric defaults. Compaction deletes only complete summaries, atomically advances a persisted history floor, and never removes the current revision. The floor is the oldest revision a client may supply and still receive every subsequent summary; the summary that created that floor revision need not be retained.

`TooOld` is a normal, expected response meaning "discard local state and re-read." `unknown_revision` also forces resync and covers tokens not belonging to retained history. It is not acceptable to guess ordering from the opaque client token.

The critical rule: **a client MUST NEVER receive an incomplete delta.** Before reading summaries, `changesSince` validates the requested token against the retained revision index/floor; any missing revision or gap returns `TooOld`. An incomplete delta is indistinguishable from a complete one, so the client silently corrupts its model and has no way to detect it. Returning `TooOld` and forcing a resync is always correct; guessing is never.

Revision-based synchronization rather than event streaming is what keeps stateless, reconnecting, and short-lived clients correct with no session state.

### Root identity and relocation

`RootId` is the stable logical key for a configured root. It is supplied by the
caller or persisted configuration and is never regenerated from an absolute
path, path spelling, label, or current canonicalization. The persisted root
record retains the last bound path and enough private physical identity data
for diagnostics; neither is part of `NodeId`, `EdgeId`, or public output.

On activation:

1. A new `RootId` creates a new root record after normal containment and
   permission checks.
2. The same `RootId` bound to a different canonical path is an explicit
   relocation. The engine preserves root-relative identity, emits a
   `ROOT_RELOCATED` diagnostic, and runs a full reconciliation scan of that
   root before reporting it current.
3. The same `RootId` and path with a changed physical directory identity is
   treated as a replacement/rebind, not as proof that old bytes remain. It
   follows the same full-scan path and may produce ordinary file deletes/adds.
4. A caller that wants two logical roots at one path must use distinct IDs and
   receives a configuration diagnostic if their selection would double-index
   the same state without an explicit separate state namespace.

The engine never guesses that two different IDs refer to the same relocated
root. Relocation can preserve node IDs when relative paths and identity inputs
remain the same; it never suppresses reconciliation or converts a path move
into a rename claim.

## 12. File selection

The engine needs its own traversal rules; it cannot rely on a client's, because it also runs from a CLI and the local daemon. These rules are a safety boundary ([Safety and Data Handling §2](safety.md#2-exclusions)), distinct from query scope.

- Honor the version-control ignore file and an engine-specific ignore file. The engine's own state directory is always ignored.
- Detect binary content by sniffing, not extension alone.
- Enforce a maximum file size with a conservative default.
- Do not follow symlinks escaping the configured roots; track visited real paths to terminate cycles.
- Support multiple roots. Store paths as `(root, relativePath)` so a root can be relocated without rewriting every path — which also makes multi-package and monorepo layouts a configuration matter rather than a special case.
- Never traverse outside configured roots.

**Every skip is recorded with a reason and is queryable.**

```text
Skip
  root:   RootId
  path:   Path
  reason: excluded | binary | too_large | unreadable | parse_failed | unsupported_language
```

Silent skipping makes coverage reporting a lie. A caller deciding whether `not_found` means anything needs to know that 400 files were skipped, and why.

## 13. Watching

File watching is an engine feature, not a client's job — a client that implements watching must reimplement normalization, debouncing, and coalescing, and will do it differently.

```text
raw platform events
    ↓ normalize      per-platform semantics erased
    ↓ debounce       settle bursts
    ↓ coalesce       collapse per-path sequences
    ↓ deduplicate    content-hash comparison
    ↓ updateFiles
```

The `Watch` port hides platform notification semantics entirely; clients see only normalized `FileChange` values.

Coalescing example — an editor's atomic-save pattern:

```text
WRITE  foo
WRITE  foo
RENAME tmp -> foo
WRITE  foo
        ↓
Modify(foo)
```

Debounce defaults to a short interval (tens of milliseconds). **Correctness MUST NOT depend on the debounce duration**; it is a latency-versus-batching tuning knob only. Any logic that only works at a particular debounce value is a bug that will surface on a slower filesystem.

Watching is optional. Explicit notification and scanning are complete alternatives, and a client that provides neither still gets a correct graph from `update()`.

## 14. Freshness and semantic lag

The deterministic graph and any semantic index advance independently.

```text
                ┌── deterministic graph ──> revision N   (immediately usable)
file change ────┤
                └── semantic queue ──> embeddings ──> semantic revision N-k
```

- A deterministic revision MUST NEVER block on embedding, model, or network work.
- Status reports both revisions and the pending count, so a caller can see the lag rather than infer it.
- Semantic lag may surface stale *content* in semantic results. It MUST NOT surface dangling node references ([Graph Model §9](graph-model.md#9-deletion-semantics)).

Detail in [Optional Intelligence](intelligence.md).
