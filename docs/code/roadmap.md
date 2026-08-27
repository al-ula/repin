# Roadmap

Repin moves through four project lifecycle stages:

```text
planning → research and analysis → plan finalization → implementation
```

The lifecycle stages are sequential decision gates, not calendar estimates. Capability milestones within implementation are ordered by dependency and risk, not by schedule.

## Stage 1 — Planning *(concluded)*

Planning defined the product, vocabulary, and architectural boundaries without committing prematurely to unverified technologies.

Delivered:

- Defined requirements, non-goals, vocabulary, and architectural boundaries.
- Specified the graph model, identity, provenance, revisions, and consistency invariants.
- Specified public APIs, result normalization, degradation, safety, and budgeting.
- Specified port contracts without binding core logic to concrete products.
- Identified implementation candidates and evaluated architectural fit.
- Formulated research questions with explicit evaluation criteria and validation tasks.
- Established strict separation of required deterministic capabilities from optional intelligence.

All architecture documents are internally consistent, and required and optional capabilities are strictly separated.

## Stage 2 — Research and analysis *(concluded)*

Research and analysis compared implementation candidates against the contracts and constraints selected during planning.

Delivered:

- Mapped each candidate to the applicable port contract and decision criteria.
- Conducted primary documentation, source, release history, and issue reviews for storage, search, runtime, and parsing candidates.
- Produced detailed comparative analyses: [redb + Tantivy versus SQLite + FTS5](research/redb-tantivy-vs-sqlite.md) and [libSQL embedded-local](research/libsql-embedded-local.md).
- Formulated subsystem proposals: [sparse line index](specifications/sparse-line-index.md), [native parsers with Tree-sitter fallback](specifications/native-parsers-tree-sitter-fallback.md), [Rust-friendly vector baseline](specifications/vector-search-rust-friendly.md), and [agent inspection & review context](specifications/agent-inspection-and-review-context.md).
- Classified candidates, established confidence levels, and identified required implementation-validation tasks.

## Stage 3 — Plan finalization *(finalized and accepted)*

Plan finalization converted research findings into 23 accepted Architectural Decision Records (ADRs). It serves as the bridge between architectural research and production implementation.

Delivered:

- Accepted 23 definitive ADRs recorded in the [Decision Ledger](decisions/index.md) (ADR-001 through ADR-023).
- Finalized the implementation profile: Rust 2024, bundled SQLite 3.53.2 via rusqlite 0.40.1 with FTS5, Linux pathname Unix sockets per-user daemon, native parsers with Tree-sitter fallback, `regex` direct search, bounded Git subprocess VCS, and sparse-checkpoint line index.
- Established the exact Rust vector search baseline for I5 (ADR-012).
- Defined the agent inspection and review context API profile (ADR-016).
- Finalized crate/module boundaries, the composition root, and the daemon rendezvous model (ADR-015).
- Finalized on-disk revision, transaction isolation, migration, and rebuild protocols.
- Mapped port conformance suites and invariants to concrete implementation milestones.

All decisions blocking deterministic implementation are resolved. The implementation plan is actionable without further architectural discovery.

## Stage 4 — Implementation *(delivered core profile)*

Implementation builds and validates the product according to the finalized plan. The core implementation profile is validated with conformance suites, replay convergence testing, and the CLI frontend. Capability milestones below describe the product capability profile; the reusable-library extraction is governed by the M0–M10 sequence below.

## Stage 5 — Reusable library extraction *(ADR-023 delivered; modular hub-and-spoke adopted in ADR-031)*

The extraction preserved the existing product and protocol while making
indexing, retrieval, context construction, and optional intelligence usable by
embedded Rust applications. Capability contracts remain those of
[ADR-023](decisions/ADR-023-reusable-library-crates.md). Workspace packaging
was structured into a modular hub-and-spoke topology by [ADR-031](decisions/ADR-031-modular-hub-and-spoke-architecture.md):
`repin-core` is the zero-heavy-dependency contract hub, leaf capability adapters are isolated spoke crates, `repin-runtime` serves as the composition root, and product layout, daemon, CLI, and executable are consolidated in `repin`.

| Milestone | Deliverable | Exit evidence |
| --- | --- | --- |
| M0 | Stabilized ADR-022 provider contracts and failure taxonomy | offline tests plus recorded tier smoke results |
| M1 | Correctness, ordering, I/O, allocation, and latency baselines | reproducible fixtures, commands, and snapshots |
| M2 | Accepted [ADR-023](decisions/ADR-023-reusable-library-crates.md) and cross-referenced docs | `mdbook build docs/code` and link checks |
| M3 | Reusable source/snapshot filesystem contract | source conformance and graph-free direct retrieval |
| M4 | context module | graph-free/enriched golden context and exact budgets |
| M5 | retrieval module | SQLite/test-store retrieval and unchanged canonical ordering |
| M6 | indexing module | clean/incremental replay convergence and atomic updates |
| M7 | intelligence module | provider contract tests and explicit offline absence |
| M8 | `Runtime` composition with `Engine` alias | unchanged daemon/CLI behavior |
| M9 | embedded RAG proof with caller-owned inference | offline fake-model test and opt-in local smoke |
| M10 | publication readiness | workspace, conformance, docs, feature, and benchmark gates |
| M11 | [ADR-029](decisions/ADR-029-consolidated-crate-topology.md) five-crate workspace | `cargo metadata` lists only the five members; `cargo test --workspace` |
| M12 | [ADR-030](decisions/ADR-030-two-crate-workspace-topology.md) two-crate workspace | `cargo metadata` lists only `repin-core` and `repin`; `cargo test --workspace` |
| M13 | [ADR-031](decisions/ADR-031-modular-hub-and-spoke-architecture.md) 11-crate hub-and-spoke | `cargo metadata` lists the 11 workspace members; `cargo test --workspace` |

The public contract hub is `repin-core`, and the default composition root is `repin-runtime`. The default build remains offline and
deterministic. The extraction acceptance budget is at most 5% median and p95
regression after variance analysis, with zero added in-process serialization,
store round trips, or source reads.

## Implementation capability milestones

### I0 — Foundations

Decisions and foundations that are structurally expensive to change later. Everything here blocks I1 in the sense that building on a wrong answer means rebuilding.

- Node and edge **identity scheme** ([Graph Model §4](graph-model.md#4-identity)), with stability tests before anything depends on it.
- **Kind and attribute registries**, and their version records.
- **Port contracts** and their conformance suites ([Conformance §2](conformance.md#2-port-conformance-suites)), with at least one implementation of each required port.
- Recorded Rust build inputs: pinned lockfile, stable compiler, dependency versions/sources/features, advisory state, and native-component inventory.
- **Unresolved-reference table** and its reverse index ([Incremental Updates §8](incremental.md#8-unresolved-references)).
- **Result envelope**, evidence rules, and error taxonomy ([Results and Evidence](results.md)).
- **Selection and exclusion rules** ([Safety and Data Handling §2](safety.md#2-exclusions)), including the secret-bearing defaults.
- Fixture repositories, golden-snapshot mechanism, and the **replay harness** ([Conformance §1](conformance.md#1-invariants)).
- Version records and the migration/rebuild decision table ([Storage §3](storage.md#3-version-records)).
- **Runtime identity and rendezvous contracts**: one daemon lease per OS user, private runtime directory, pathname Unix-domain socket, bounded startup race, and protocol negotiation ([Runtime and IPC](runtime.md), [ADR-015](decisions/ADR-015-hybrid-per-user-daemon-runtime.md)). Build the deterministic engine and port conformance in process first, then complete the daemon wrapper before normal CLI and multi-client behavior.
- **Project discovery and state safety**: `.repin/graph.sqlite3` membership, canonical parent-directory resolution, explicit-root selection, active filesystem-identity alias guards, and the runtime error taxonomy.
- **Context registry and lock ownership** keyed by the canonical database path, with the daemon holding each active project's writer lock and clients never acquiring it.

Exit:

```text
port conformance suites pass against real implementations
replay harness runs and passes on trivial fixtures
node ids survive edits above them
excluded categories are provably unindexed
```

The replay harness exists in I0 rather than later because retrofitting convergence testing onto a built engine means discovering divergence long after the cause.

### I1 — Deterministic engine

A working, useful, index-once engine.

- Graph model, store, transactions, revisions.
- File discovery with ignore, binary, size, symlink, and multi-root rules.
- Language detection; one or two language packs plus a prose pack.
- Extraction via batch-shaped queries; local fact extraction using native primary parsers with Tree-sitter fallback ([ADR-013](decisions/ADR-013-native-parser-tree-sitter-fallback.md)).
- Sparse-checkpoint line index for bounded byte-offset to line/scalar coordinate conversion ([ADR-014](decisions/ADR-014-sparse-checkpoint-line-index.md)).
- Cross-file resolution with unresolved-reference recording.
- Lexical index and text search via SQLite FTS5 in the same transaction domain ([ADR-009](decisions/ADR-009-sqlite-fts5-initial-profile.md)).
- Graph traversal: entity, neighbors.
- Direct retrieval: files, text, regex via `regex` crate ([ADR-010](decisions/ADR-010-regex-direct-search.md)).
- Deterministic inspection primitives: `inspectFile` (syntax-only outline), `AtPosition` position resolution, and `context(strategy: exact)` reads ([ADR-016](decisions/ADR-016-agent-inspection-and-review-context.md)).
- Skips and diagnostics, queryable.
- CLI covering initialize, update, search, entity, neighbors, status, stats.
- Project-bound CLI client and daemon round trip: initialization, nearest ancestor discovery, explicit root selection, context sharing, observer attachment, degraded direct retrieval, and protocol error reporting.
- Context lifecycle: watcher/update coordinator ownership, bounded connection handling, ten-minute idle eviction, daemon restart, and clean final shutdown.

Exit:

```text
a repository indexes, persists, and reopens without rebuilding
text and symbol search return evidence-backed results
coverage reporting is honest about what was skipped
graph well-formedness holds after every commit
```

### I2 — Incremental updates

The milestone that makes the engine practical rather than a demonstration.

- `FileChange` model with origin, and content-hash deduplication.
- `updateFiles` as the primitive.
- Invalidation with blast-radius classification.
- Unresolved-reference promotion and demotion.
- Transactional updates with reader isolation.
- VCS-based startup change detection via bounded Git subprocess ([ADR-011](decisions/ADR-011-bounded-git-subprocess.md)).
- Backpressure, bulk escalation, pause/resume.
- Revisions with bounded retention, `changesSince`, and `TooOld`.
- Single-writer enforcement with reader mode.

Exit:

```text
single-file edits update without full rebuild
convergence holds over long generated change sequences
order independence and restartability hold
a branch switch escalates rather than thrashing
a second context enters observer/direct-only mode and says so
```

Convergence is the exit criterion that matters. The others are prerequisites for testing it.

### I3 — Watching

- Watch port with platform-specific backends behind it.
- Event normalization, debounce, coalescing, deduplication.
- Managed watch session with lifecycle.
- Correctness independent of debounce duration.

Exit:

```text
external edits become queryable automatically
editor atomic-save patterns produce one logical update
no lost events under burst load
behavior identical at several debounce values
```

### I4 — Retrieval quality

Where the engine becomes genuinely useful rather than merely correct.

- Symbol, structural, and graph-aware retrieval channels.
- Channel merge and result fusion.
- Deterministic ranking with explanations.
- Filters, including `derivation`.
- Trace and impact, bounded, with coverage.
- Context construction with strategies and budgets.
- Agent inspection & review context: graph-enriched `inspectFile`, `reviewContext` composition over change diffs and impact, and identifier-aware lexical tokenization ([ADR-016](decisions/ADR-016-agent-inspection-and-review-context.md)).
- Labeled query set and precision-at-N measurement.

Exit:

```text
precision at N is measured and tracked, not asserted
every result can explain its rank
trace returns ordered paths and distinguishes its three outcomes
impact results are grouped, bounded, and never claimed exhaustive
context respects budgets exactly and reports omissions
```

This is the milestone where the engine should be evaluated on real repositories, before any model capability exists to mask weak retrieval.

### I5 — Embeddings

- `EmbeddingModel` port; local and remote providers.
- Exact Rust Vector-port baseline from ADR-012 and semantic channel.
- Entity rendering, deterministic chunking, normalization.
- Embedding cache with the full key.
- Semantic revision tracking and status reporting.
- Hybrid fusion into the existing merge.

Exit:

```text
semantic indexing cannot delay a deterministic revision
a provider outage degrades recall only
cache hit rate is reported and high across reformatting
deleted nodes never surface as semantic hits
precision at N improves measurably over I4, or the channel is not enabled by default
```

The last criterion is the point of sequencing embeddings after I4: without a deterministic baseline, there is nothing to demonstrate improvement against.

### I6 — Reranking

- `Reranker` port; local, remote, and host-supplied providers.
- Candidate cutoff and rerank window.
- Deadline bounding with deterministic fallback.

Exit:

```text
reranker failure returns deterministic order as ok, not partial
reranking cannot introduce entities retrieval did not find
deterministic explanations survive reranking
precision at N measured with and without
```

### I7 — Host integration

- Provider-contract adapter for hosts with their own vocabulary.
- Direct change notification from a host.
- Host-supplied model adapter.
- Capability negotiation and freshness surfacing.
- Context-budget integration with caller-supplied estimators.

Exit:

```text
a host adapter needs no privileged access
host edits are queryable without waiting for the watcher
host and watcher reports for one write produce one update
the engine core contains no host-specific vocabulary
```

Integration is late by design. Every earlier phase is a capability a host can use; building the adapter first would shape the engine around one consumer.

### I8 — Enrichment

Only after deterministic retrieval is mature and measured.

- `TextModel` port.
- Derived relations, stored separately.
- Independent deletion and rebuild.
- `derivation` filtering end to end.

Exit:

```text
discarding all enrichment leaves a valid graph
no deterministic fact depends on an inferred one
inferred facts are distinguishable at every layer
enrichment measurably improves precision at N, or stays disabled
```

## Initial implementation scope

The first useful prototype deliberately omits embeddings, generation, enrichment, deep language-service integration, and distributed operation. It builds:

```text
deterministic engine + persistent graph + batch extraction
+ lexical and symbol search + graph traversal
+ transactional incremental updates + unresolved resolution
+ user daemon + local IPC + project-bound CLI client
+ project discovery + isolated contexts + per-project writer ownership
+ watcher + idle lifecycle + degraded direct retrieval
```

Then it is benchmarked on real repositories before any model capability is added. Adding intelligence to unmeasured retrieval makes the retrieval permanently unmeasurable.

## v1 definition of done

Standalone v1 is complete when the engine can:

**Index and persist**

- initialize a repository and index it deterministically
- persist and reopen without rebuilding
- migrate, scope-invalidate, or rebuild on version mismatch
- refuse to open a graph written by a newer engine

**Update correctly**

- detect changes, using version control when available
- update incrementally without a full rebuild
- converge: incremental application equals a fresh index
- resolve forward references as definitions appear
- survive branch switches and bulk changes by escalating
- expose revisions and report changes since one, or `TooOld`

**Answer usefully**

- search by text, pattern, and symbol
- resolve entities with explicit ambiguity
- traverse relationships, trace bounded paths, estimate impact
- construct budgeted context
- explain ranking decisions
- report coverage honestly, including skips and unresolved references

**Behave safely**

- contain every path within configured roots
- exclude secret-bearing content by default
- redact credential-shaped content from all output
- never execute repository content
- bound every operation and record every degradation

**Integrate cleanly**

- work with direct retrieval when no graph exists or graph activation is unavailable
- start one user daemon on demand and bind each client connection to one project context
- enforce per-project writer safety through daemon-owned locks across all clients
- run entirely offline
- expose a stable public API with opaque identifiers
- support a host provider contract and the local daemon protocol without core changes

AI capabilities are then additive layers, each independently absent.
