# Conformance

What must be true, and how it is verified. A deterministic engine is testable in ways an AI-dependent one is not, and this document exists to exploit that fully.

## 1. Invariants

Each is mechanically checkable, and each has a named suite.

### Convergence

```text
fresh_index(final)  ==  apply_changes(fresh_index(initial), sequence)
```

The central property ([Incremental Updates §1](incremental.md#1-the-convergence-invariant)). Nodes, edges, resolution outcomes, and the unresolved set compare exactly under [Graph equality](#graph-equality).

Verified by a **replay harness**:

1. build a fixture repository at an initial state
2. index it fresh
3. apply a generated change sequence incrementally
4. index the final state fresh in a separate store
5. compare graphs exactly under [Graph equality](#graph-equality)
6. additionally compare after every individual step

Generated sequences cover create, modify, delete, rename, move, reorder within a file, whitespace-only edit, branch switch, bulk change, and interleavings. Sequences are seeded and reproducible; a failing seed becomes a permanent regression case.

Also asserted: **order independence** (permuted sequences converge identically) and **restartability** (interrupting and resuming converges).

### Graph equality

Several suites compare two independently built graphs — convergence, restartability, rebuild, and recovery. They all use one canonicalization so that "the same graph" means the same thing everywhere.

Canonical comparison compares exactly:

- the node set with `kind`, `name`, `qualifiedName`, `language`, `artifactClass`, and each node's stable identity
- the edge set with its stable `EdgeId`, `from`, `to`, and `kind`, so an incorrect identity derivation cannot pass by matching endpoints alone
- the unresolved-reference set with `from`, `edgeKind`, seeking text, scope hint, and referencing owner, because two references differing in source or edge kind promote to different edges
- the skip and diagnostic sets
- the per-owner claim set backing every canonical fact, so a fact supported by two producers is never equal to the same fact supported by one
- `derivation` and `confidence`, so an inferred fact never compares equal to an extracted one
- each fact's owning `root`, `path`, `extractor`, and `extractorVersion`

Canonical comparison ignores:

- revisions, timestamps, and durations
- internal iteration, storage, and result ordering
- adapter-internal representations, including tombstones and physical layout
- **the provenance representative of a multi-occurrence fact.** When several occurrences collapse into one canonical node or edge, that fact carries one optional `range`, and which occurrence supplies it is an adapter choice. Equality compares the fact and its owning claims, but not which occurrence's `range` won. A differing representative is reported as an observation, never as an inequality.

Everything not listed as ignored is compared. In particular, a single-occurrence fact's `range` is compared exactly, so this rule cannot hide range defects. Comparison is symmetric and must fail on both missing and extra elements; a comparison that only checks containment is not a graph-equality check.

Occurrence counts are deliberately **not** part of portable equality. The canonical graph exposes one fact per identity, and no port operation reports how many occurrences produced it. A suite that wants occurrence-level evidence may obtain it from an adapter's own test-only inspection surface, and must record the absence of that surface as a coverage limitation rather than a failure.

The same canonicalization applies to any future fixture oracle, so a generator oracle and a rebuild comparison cannot disagree about what equality means.

### Identity stability

- Editing above an entity does not change its id.
- Reformatting a file changes no ids.
- Moving an entity within the same named container preserves its id; `range` changes.
- Moving an entity across named containers changes its id because its stable address changed.
- Same-named siblings receive stable, reproducible discriminators.
- `unstableId` nodes are flagged and never used as cache keys.

This suite exists because identity failures are silent: everything works, only slower and with useless change reports.

### Graph well-formedness

The eight invariants in [Graph Model §8](graph-model.md#8-graph-invariants), checked after every commit in test mode:

no dangling endpoints, provenance revision within bounds, paths contained in roots, kinds registered, `contains` forms a forest, ids unique, nodes rooted except externals, and no deterministic fact depending on an inferred one.

### Transactional isolation

- A reader concurrent with a commit observes exactly the old or the new revision.
- A failed authoritative commit leaves the previous graph revision fully intact.
- A graph revision exists if and only if its authoritative commit succeeded.
- A separate lexical commit failure leaves the new graph revision valid, reports lag, and converges through idempotent repair or rebuild.
- Concurrent update requests coalesce rather than queueing transactions.

### Change history

- Every authoritative commit atomically persists exactly one normalized `UpdateSummary`; failed/no-op commits persist none.
- `changesSince` returns a complete contiguous delta or `TooOld`. A missing summary/gap is injected in tests and MUST produce `TooOld`, never a shorter apparent success.
- Compaction removes whole summaries, atomically advances the history floor, and never removes the current revision.
- Entry-count and age policies are tested independently and together across reopen.
- Revisions are never reused, including across compaction and migration.

### Stale-plan conflicts

- A prepared plan invalidated by revision/content change is never committed.
- At most two automatic reprepare attempts occur per API call and remain within its original cancellation/deadline budget.
- Exhaustion returns `UPDATE_CONFLICT`, retains/coalesces affected roots for reconciliation, and exposes no partial revision.
- Continuous churn cannot create an unbounded retry loop or starve unrelated queued work.

### Resolution convergence

- A definition added after a reference promotes the unresolved reference.
- A definition removed demotes the edge back to unresolved, retaining `seeking`.
- Add-then-remove-then-re-add returns to the same graph.
- Unresolved counts are accurate and match a full recount.

### Selection and safety

Enumerated in [Safety and Data Handling](safety.md); verified adversarially in [§4](#4-adversarial-tests).

### Runtime and IPC

The runtime suite must verify the process and context invariants in
[Runtime and IPC](runtime.md). ADR-015 accepts the topology while leaving these
checks as required implementation validation:

- concurrent cold-start clients elect exactly one daemon and losing candidates
  reconnect;
- nearest-ancestor discovery, explicit-root override, incomplete markers, and
  canonical parent-directory resolution produce the specified selector result;
- two connections share one canonical-path context and revision, while copied
  databases and their pending work remain isolated;
- active symlink, rename, bind-mount, hard-link, and alternate-spelling aliases
  cannot create a second context;
- invalid/newer graph state preserves bounded direct retrieval with explicit
  graph-unavailable status, and external lock ownership produces observer mode
  with `PROJECT_LEASE_UNAVAILABLE` for writes;
- virtual-clock idle eviction occurs at `600,000 ms` only when the context is
  truly idle, and final context unload precedes daemon socket close and lease
  release;
- daemon death releases project locks, restart repairs stale rendezvous state,
  and closing one client leaves unrelated work unaffected;
- protocol negotiation, request IDs, progress, deadlines, cancellation, and
  bounded admission work across the bound connection.

### Degradation

For each optional port, absent and failing:

- no store: direct retrieval still answers
- no lexical port: text search falls back to scan
- lagging lexical port: it is bypassed; a current direct scan substitutes where supported, the bypass is warned, and coverage is partial only when the fallback cannot completely search the scope
- failing lexical port with unknown freshness: graph remains authoritative, suspect hits are not trusted as current evidence, and direct fallback/coverage behavior is explicit
- no vector port: semantic omitted, deterministic unaffected
- no watch port: explicit notification and scan still work
- no vcs port: full scan on startup
- no packs for a language: text-only, file still indexed
- embedding provider failing: deterministic revisions continue advancing
- reranker failing: deterministic order returned as `ok`

The last two are the ones most likely to regress, because a working provider hides the fallback path.

## 2. Port conformance suites

Every port has one suite that every implementation must pass. This is what keeps a port a port rather than a de facto dependency on one product.

**Store** — atomicity, rollback, snapshot isolation, durability across reopen, batched owner-claim writes, owner/file-scoped removal without deleting another claim, deterministic canonical materialization, bidirectional edge traversal, unresolved reverse lookup, atomic normalized history summaries/floor compaction, version records, migration, writer exclusivity, reader concurrency.

**Lexical** — batched indexing, removal by key, filtered query, match regions present and accurate, ranking stability, incremental update correctness, revision-lag detection, coordinator bypass of known-lagging state, current direct-scan substitution and coverage reporting, and idempotent repair/rebuild.

**Vector** — upsert, acknowledged deletion remains absent from search after reopen (regardless of internal tombstone representation), metadata filtering, dimension enforcement, result ordering by distance.

**Watch** — event normalization across platforms, editor atomic-save coalescing, rapid-burst coalescing, rename detection, deduplication by content, correctness independent of debounce duration, no lost events under load.

**Vcs** — changed-set accuracy, branch-switch detection, unreachable-revision fallback, dirty-file handling, shallow-clone fallback.

**LanguagePack** — detection determinism, extractor purity (no IO, no shared state), extraction determinism, batch-shaped output, bounded failure, no handle escape, capability honesty.

Two suites deserve emphasis. **Vector deletion**: after acknowledged deletion, a key must not be externally visible, including after reopen; unfiltered tombstones look like relevance noise rather than a bug. **Watch debounce independence**: logic that only works at one debounce value passes locally and fails on a slower filesystem.

## 3. Golden snapshots

Fixture repositories with committed expected graphs, per language pack.

- A snapshot records nodes, edges, provenance derivation, and the unresolved set — not internal ids, which are permitted to change.
- Snapshot diffs make grammar and pack upgrades **reviewable** instead of mysterious. This is the primary defense against a parser upgrade silently changing what gets indexed.
- **Separate snapshots per parser binding.** Different builds of nominally the same grammar can produce materially different node sets, so a fallback parser is never assumed equivalent to the primary one ([Extraction §6](extraction.md#6-versioning)).
- Updating a snapshot requires review of the diff. An auto-regenerated snapshot verifies nothing.

## 4. Adversarial tests

Safety rules are only real if they are attacked.

**Paths** — traversal sequences, absolute paths outside roots, symlinks escaping roots, symlink cycles, symlink swapped between check and read, unicode and encoding tricks in path components, very long paths.

**Content** — deeply nested structures, extremely long lines, files with no line breaks, invalid encodings, binary content with source extensions, pathologically large generated files, content engineered to trigger worst-case parser behavior.

**Instruction-shaped content** — file content that reads as directives to an automated consumer must carry no authority anywhere in the pipeline.

**Provider responses** — paths outside roots, malformed ranges, oversized previews, credential-shaped previews, ids that do not exist, inconsistent capability declarations.

**Secrets** — every excluded category is confirmed unindexed, unpreviewd, and never transmitted to a provider; credential-shaped values are confirmed redacted from previews, logs, errors, and diagnostics.

**Resources** — oversized queries, catastrophic regex patterns, unbounded traversal requests, queue flooding, memory pressure. Each must degrade with a recorded reason rather than fail the engine.

## 5. Quality measurement

Replaces the unmeasurable claim "useful without a model."

- A committed labeled query set: query paired with entities that should appear in the top N.
- Precision at N tracked over time; ranking changes evaluated against it before merging.
- Stability matters more than size. A query set edited to match new behavior measures nothing, so changes to it require separate review from changes to ranking.
- Reported per channel configuration — deterministic only, plus semantic, plus rerank — so each layer's actual contribution is visible rather than assumed.

## 6. Benchmark method

Numbers without method are not evidence.

**Corpus.** Future evidence runs should use versioned Micro, Small, Medium, Large normal, and Pathological bands. The prior target was 10,000 selected files and 250 MiB of selected content; generated, dependency, and vendor trees are excluded normally and measured in bounded pathological cases. Composition is documented, because corpus composition dominates results—a corpus heavy in declaration-only files measures something quite different from real implementation code. A corpus is classified by its highest file, byte, node, or edge band, and reported measurements use actual counts.

**Phases measured separately.** Crawl, read, hash, detect, parse, extract, resolve, store write, lexical index, vector index. Aggregate-only numbers hide which phase actually costs.

**Resolution measured explicitly.** It does not parallelize the way extraction does and is the phase most likely to dominate at scale. Assuming it is cheap is the most common estimation error in engines of this kind.

**Reported with every result:** reference hardware, storage medium and filesystem, corpus version and composition, warm or cold state, concurrency, and run count with variance.

**Storage medium is not incidental.** Benchmarks on a memory-backed filesystem overstate write throughput substantially and must be labeled as such; results are not comparable across media.

**Extrapolation is labeled as extrapolation.** A projection from a small corpus to a large one is a hypothesis until measured, and linear scaling is a poor assumption for the resolution phase specifically.

Results are stored so regressions are visible across commits. Linux
x86_64/glibc runs all benchmark bands and qualification cases for the current
PoC. macOS, Windows, musl/static Linux, and other architectures are excluded
from the current implementation and become a separate post-PoC validation
phase; Linux measurements do not imply portability to them.

## 7. Test taxonomy

| Level | Scope | Requires |
|---|---|---|
| unit | identity, classification, coalescing, ranking, normalization, redaction | nothing |
| port | one port against its suite | one implementation |
| integration | full pipeline over fixtures | all required ports |
| convergence | replay harness | full pipeline |
| adversarial | safety attacks | full pipeline |
| quality | precision at N | labeled query set |
| benchmark | throughput and latency | fixed corpus |

Every level runs under each configuration in the current Linux PoC scope:
each store implementation, parser binding, and host environment. A feature
verified on Linux is verified for Linux only. Once the fully featured PoC is
complete, the platform-expansion phase will add separate build, behavior, and
artifact matrices rather than inferring them from Linux.

## 8. Reusable library and extraction checks

The crate extraction adds gates without weakening the existing product suites:

- `SourceFs` implementations pass containment, exclusion, classification,
  size/binary, symlink, cancellation, and error-semantics checks. A lightweight
  test source is required in addition to `CapabilityFs`.
- Context tests cover graph-free and graph-enriched packing, exact byte/line
  and unit-budget behavior, deduplication, canonical order, redaction, and
  explicit omission/truncation reporting.
- Retrieval tests cover direct, graph, lexical, vector, and hybrid channels;
  absence of a graph/store never disables direct retrieval, and canonical
  result ordering matches the pre-extraction golden output.
- Indexing tests compare clean and incremental replay convergence,
  transaction atomicity, unresolved-reference promotion/demotion, and
  revision checks through the reusable contracts.
- Intelligence tests use deterministic fake models, malformed responses,
  deadline timeouts, unavailable providers, and the recorded provider smoke
  commands. No credentials or network access are required by default.
- Cargo metadata must show the dependency direction
  `capability crates -> repin-runtime -> repin-engine -> product frontends`;
  `repin-runtime -> repin-engine` is a failure.
- Existing facade compile fixtures, daemon protocol snapshots, CLI contract
  tests, and serialized envelopes must remain unchanged.

The extraction is accepted only when these checks pass with no added
in-process serialization, store round trips, or source reads in an existing
operation. Deterministic median and p95 regressions are compared with the
baseline under the 5% ADR-023 budget after variance analysis.
# Compatibility versioning (ADR-024)

The conformance suite MUST verify owning-crate constants and JSON diagnostics;
highest-common protocol selection and bounded bootstrap handling; SQLite
identity inspection before DDL, empty-versus-existing version-zero
classification, unrelated/newer/corrupt/contradictory state handling, and
transactional migration/version-record preservation; complete scoped
invalidation for registry, classification, resolution, pack, and extractor
changes; and conservative replacement with full-idle detection, singleton
election, and actionable busy-daemon mismatch recovery.

Recovery target vocabulary (`graph`, `lexical`, `vector`, `all`) must remain
consistent across CLI and IPC; unavailable derived-index adapters must return
explicit capability errors.

The protocol suite also verifies the 1 MiB ordinary-frame and 64 KiB bootstrap
limits and the 2,000 ms bootstrap deadline, including rejection before project
binding.

The lifecycle suite verifies that detached contexts remain isolated while
within the ten-minute idle window, then release their project writer lease on
reaping; a daemon replacement request is eligible only after that reaping and
with no other bootstrap/client connection.

Producer-version invalidation must exercise owner discovery across nodes,
edges, unresolved references, skips, and diagnostics, verify that no source
read occurs during removal, and confirm that replacement `VersionRecords` and
the removal revision become visible together.

Classification conformance must verify batched node-claim updates, no source
reads, and atomic publication of the classification version record.

The storage suite also includes the adopted v1-to-v2 migration, verifies that
ordinary open refuses v1, verifies journal and version-record stamping after
explicit migration, and injects a malformed version record to prove rollback
leaves the v1 schema untouched.
