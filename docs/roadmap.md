# Roadmap

Repin moves through four project lifecycle stages:

```text
planning → experimentation → plan finalization → implementation
```

The lifecycle stages are sequential decision gates, not calendar estimates. Capability milestones within implementation are also ordered by dependency and risk, not by schedule.

## Stage 1 — Planning *(current)*

Planning defines the product and selects hypotheses to test. It does not commit to production technologies or produce implementation code.

Work:

- Define requirements, non-goals, vocabulary, and architectural boundaries.
- Specify the graph model, identity, provenance, revisions, and consistency invariants.
- Specify public APIs, result normalization, degradation, safety, and budgeting.
- Specify port contracts without binding core logic to products.
- Identify implementation candidates and the reasons they might fit.
- Turn uncertainties into reproducible experiments with explicit pass conditions.
- Separate required deterministic capabilities from optional intelligence.

Current technology candidates are recorded in [Technology Candidates](technology-candidates.md). Planned storage experiments are specified in [Storage Adapter Experiments](experiments/storage.md); extraction, filesystem, runtime, watch, and engineering-tool experiments are specified in [Rust Foundation Experiments](experiments/rust-foundation.md).

Exit:

```text
architecture documents are internally consistent
required and optional capabilities are clearly separated
candidate choices are labeled proposed rather than accepted
blocking uncertainties have experiments and pass conditions
no unresolved architecture issue prevents experimentation
```

## Stage 2 — Experimentation

Experimentation tests the hypotheses selected during planning. Spikes are disposable and MUST NOT silently become production foundations.

Work:

- Run candidate adapters against the applicable port conformance behaviors.
- Exercise failure, interruption, deletion, reopen, repair, and rebuild paths.
- Benchmark representative repository and graph shapes using the method in [Conformance](conformance.md).
- Run full qualification on Linux x86_64/glibc, the sole current PoC target.
- Build and smoke-test the Linux PoC artifact. Defer non-Linux artifacts, minimum-version work, and distribution expansion until the fully featured PoC is complete.
- Retain methods, raw results, failing seeds, and reproducible fixtures.
- Classify each candidate as supportable, unsuitable, or still uncertain.

The initial experiment matrix covers redb, Tantivy, optional USearch, and their combined revision/recovery protocol ([Storage Adapter Experiments](experiments/storage.md)), plus Rust extraction, filesystem, hashing, runtime, watching, testing, benchmarking, and dependency-policy foundations ([Rust Foundation Experiments](experiments/rust-foundation.md)). Further experiments may be added when planning exposes another decision that cannot be settled from documentation or existing evidence.

Exit:

```text
experiments are reproducible and results are recorded
failure and recovery behavior has been demonstrated
candidate limitations are mapped to capability or degradation behavior
each blocking candidate has enough evidence for a final decision
no benchmark claim lacks corpus and environment context
Linux PoC qualification and release-artifact smoke checks pass; post-PoC platform expansion is explicitly separated from this stage
```

## Stage 3 — Plan finalization

Plan finalization converts experimental evidence into committed implementation decisions. It is the boundary between exploring an architecture and authorizing production work.

Work:

- Accept, reject, replace, or defer each technology candidate.
- Record accepted decisions as ADRs under `docs/decisions/`.
- Finalize the implementation profile, platform-tier details and release-artifact policy, and dependency policy.
- Finalize crate/module boundaries and the composition root.
- Finalize the on-disk revision, pending-work, recovery, migration, and rebuild protocols.
- Resolve every choice that blocks the first deterministic implementation milestone.
- Map conformance tests and exit criteria to implementation work items.
- Revisit milestone scope and estimates using measured evidence.

Rust, the authoritative store, and the lexical adapter are expected to be resolved before implementation begins if they remain part of the proposed profile. A vector adapter may be explicitly deferred until the embeddings milestone.

Exit:

```text
an accepted implementation profile exists
all decisions blocking deterministic implementation are resolved
accepted products satisfy rather than reshape their port contracts
conformance and acceptance criteria map to implementation milestones
the implementation plan is actionable without architectural discovery
```

## Stage 4 — Implementation

Implementation builds and validates the product according to the finalized plan. The milestones below describe capability, not schedule. Each leaves the engine in a coherent state and has explicit exit criteria.

Experiment code is not promoted by default. Production adapters must independently satisfy their port conformance suites and the finalized support policy.

## Implementation capability milestones

### I0 — Foundations

Decisions and foundations that are structurally expensive to change later. Everything here blocks I1 in the sense that building on a wrong answer means rebuilding.

- Node and edge **identity scheme** ([Graph Model §4](graph-model.md#4-identity)), with stability tests before anything depends on it.
- **Kind and attribute registries**, and their version records.
- **Port contracts** and their conformance suites ([Conformance §2](conformance.md#2-port-conformance-suites)), with at least one implementation of each required port.
- Accepted Rust dependency policy: pinned lockfile, minimum supported Rust version, allowed licenses/sources, advisory handling, and native-component inventory.
- **Unresolved-reference table** and its reverse index ([Incremental Updates §8](incremental.md#8-unresolved-references)).
- **Result envelope**, evidence rules, and error taxonomy ([Results and Evidence](results.md)).
- **Selection and exclusion rules** ([Safety and Data Handling §2](safety.md#2-exclusions)), including the secret-bearing defaults.
- Fixture repositories, golden-snapshot mechanism, and the **replay harness** ([Conformance §1](conformance.md#1-invariants)).
- Version records and the migration/rebuild decision table ([Storage §3](storage.md#3-version-records)).
- **Runtime identity and rendezvous contracts**: one daemon lease per OS user,
  private runtime directory, pathname Unix-domain socket, bounded startup
  race, and protocol negotiation ([Runtime and IPC](runtime.md)).
- **Project discovery and state safety**: `.repin/graph.redb` membership,
  canonical parent-directory resolution, explicit-root selection, active
  filesystem-identity alias guards, and the runtime error taxonomy.
- **Context registry and lock ownership** keyed by the canonical database path,
  with the daemon holding each active project's writer lock and clients never
  acquiring it.

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
- Extraction via batch-shaped queries; local fact extraction.
- Cross-file resolution with unresolved-reference recording.
- Lexical index and text search.
- Graph traversal: entity, neighbors.
- Direct retrieval: files, text, regex.
- Skips and diagnostics, queryable.
- CLI covering initialize, update, search, entity, neighbors, status, stats.
- Project-bound CLI client and daemon round trip: initialization, nearest
  ancestor discovery, explicit root selection, context sharing, observer
  attachment, degraded direct retrieval, and protocol error reporting.
- Context lifecycle: watcher/update coordinator ownership, bounded connection
  handling, ten-minute idle eviction, daemon restart, and clean final shutdown.

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
- VCS-based startup change detection.
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
- Vector port and semantic channel.
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

### I7 — Remote and federated deployment

The local user daemon and its IPC contract are completed in I0/I1. This later
milestone is reserved for deployment beyond the local user boundary:

- Remote transport for the already project-bound protocol.
- Authentication, authorization, trust consent, and data-egress reporting.
- Federation or cross-host project coordination, if a later product decision
  requires it.
- Remote lifecycle, health, reconnect, and revision synchronization.

Exit:

```text
remote transport does not change local project or graph semantics
trust and data-egress decisions are explicit and auditable
reconnecting remote clients resync correctly via revisions
compacted revisions produce TooOld and a clean resync
```

Remote and federated operation remains deliberately later than the local
daemon: it adds authority and egress concerns without being required for a
useful offline workstation runtime.

### I8 — Host integration

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

### I9 — Enrichment

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

- work with direct retrieval when no graph exists or graph activation is
  unavailable
- start one user daemon on demand and bind each client connection to one
  project context
- enforce per-project writer safety through daemon-owned locks across all
  clients
- run entirely offline
- expose a stable public API with opaque identifiers
- support a host provider contract and the local daemon protocol without core
  changes

AI capabilities are then additive layers, each independently absent.
