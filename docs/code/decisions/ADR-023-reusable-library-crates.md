# ADR-023: Reusable capability crates and the runtime compatibility facade

```text
Status: superseded for crate topology by ADR-029; capability contracts remain accepted
Date: 2026-08-20
Decision type: crate responsibilities, dependency direction, embedded API, and extraction sequence
Builds on: ADR-002, ADR-003, ADR-005, ADR-009, ADR-012, ADR-013, ADR-015, ADR-017, ADR-018, ADR-022
Superseded in part by: ADR-029 (workspace packages; modules retain these capability boundaries)
Backs: docs/architecture.md, docs/api.md, docs/host-integration.md, docs/conformance.md
```

## 1. Context

Repin's deterministic indexing, retrieval, and context construction are useful
to an embedded RAG application even when no daemon or CLI is present. The
current product composition keeps those capabilities in `repin-engine`, which
couples reusable algorithms to the default filesystem, SQLite store, language
packs, and provider implementations. Moving code without fixing ownership
would either make embedded callers depend on product adapters or create a
runtime/facade dependency cycle.

The extraction must preserve the normative behavior already defined by the
architecture and result contracts. It is a crate-boundary change, not a new
retrieval semantics or a new storage schema.

## 2. Decision

Repin adopts the following workspace crate topology:

```text
repin-core                 domain values and port contracts
  ├─ repin-protocol        public result and wire values
  ├─ repin-direct-search   bounded working-tree scan algorithms
  ├─ repin-fs              CapabilityFs and Git adapters
  ├─ repin-store-sqlite    SQLite/FTS5 Store adapter
  ├─ repin-packs           built-in LanguagePack implementations
  ├─ repin-context         evidence validation and budget packing
  ├─ repin-retrieval       graph, lexical, vector, and ranking algorithms
  ├─ repin-indexing        extraction and transactional update orchestration
  └─ repin-intelligence    optional embedded, agent, and remote providers
             └────────────── repin-runtime (default composition root)
                              └─ repin-engine (compatibility facade)
                                  └─ repin-daemon / repin-cli
```

`repin-runtime` is the sole default composition root. It constructs
`CapabilityFs`, `SqliteStore`, `GitVcs`, the built-in packs, and configured
intelligence providers, then owns high-level operation orchestration and
result normalization. `repin-engine` depends on `repin-runtime` and retains
the existing `Engine`, `EngineOptions`, `Engine::open`, and public method
surface as compatibility aliases/re-exports during the compatibility period.

The dependency rule is strict: `repin-runtime` MUST NOT depend on
`repin-engine`. The facade may depend on the runtime; the reverse edge is
forbidden and is checked from Cargo metadata in conformance validation.

## 3. Reusable crate contracts

The capability crates depend on `repin-core` port traits and shared domain
types. They MUST NOT select concrete stores, filesystems, language packs, or
providers. In particular:

- `repin-context` accepts the specified filesystem/source contract and a
  `ReadView` (or a smaller read contract where a function needs less). Its
  packing order, validation, redaction, and budget behavior are deterministic.
- `repin-retrieval` accepts borrowed source and read-view contracts and keeps
  direct retrieval independently usable when no store is present. Vector and
  reranker calls are optional ports; failures preserve deterministic results.
- `repin-indexing` accepts `Store`, source, and `LanguagePack` contracts and
  owns invalidation, extraction coordination, resolution, and batched
  transactional update planning.
- `repin-intelligence` implements the provider-neutral model ports from
  `repin-core`. Configuration and provider selection remain in the runtime.

Traits define contracts, while generic functions and borrowed references are
used in hot loops. Trait objects are allowed at runtime-selected boundaries:
the default pack/provider registry and the runtime composition. No internal
JSON or other serialization boundary is introduced between in-process crates.

## 4. Embedded library API

An embedded consumer may depend on `repin-core`, `repin-context`,
`repin-retrieval`, and `repin-indexing` plus only the adapters it chooses. The
consumer can provide its own `SourceFs`, `Store`, `LanguagePack`,
`EmbeddingModel`, or `Reranker`. It does not need `repin-daemon` or
`repin-cli`.

The default convenience API is `repin_runtime::Runtime` with
`RuntimeOptions`; `repin_engine::Engine` remains the source-compatible facade.
The daemon and CLI continue to use the facade and the existing protocol. The
library API returns the same normalized result envelopes, provenance,
freshness, coverage, warnings, redaction, cancellation, and truncation
metadata as the service path.

Caller-owned inference is intentional. Repin may retrieve and pack context,
but it does not own conversations, prompts, generated answers, or host memory.

## 5. Feature and publication policy

New crates are `publish = false` until their APIs and feature sets stabilize.
The default feature set is offline and deterministic: no network access,
model download, or heavyweight embedded asset is enabled implicitly.

`repin-intelligence` gates concrete provider families independently where their
dependencies or assets justify it. Core, context, retrieval, and indexing
crate default builds contain no concrete provider dependency. Minimal-feature,
default-feature, and all-feature builds are checked in CI before publication.

The initial SemVer policy is workspace-wide `0.1.x`: public contracts may add
backward-compatible types and methods, while removals or semantic changes
require a decision update and a minor-version boundary. Compatibility aliases
in `repin-engine` remain until a documented major-version migration removes
them.

## 6. Migration and acceptance sequence

Extraction follows mechanical movement with compatibility re-exports:

1. capture correctness, allocation, store-call, source-read, and latency
   baselines;
2. introduce the source/snapshot contract and its conformance tests;
3. extract context, retrieval, indexing, and intelligence in that order;
4. introduce `repin-runtime`, then reduce `repin-engine` to the facade;
5. prove an embedded RAG flow with caller-owned inference; and
6. run workspace, conformance, replay, wire/golden, documentation, feature,
   and benchmark gates.

Each movement MUST preserve canonical ordering and serialized envelopes. The
accepted local regression budget is at most 5% for median and p95 deterministic
operations after variance analysis, with zero added in-process serialization,
store round trips, or source reads. A difference is acceptable only after an
explicit specification amendment.

## 7. Consequences

Embedded applications gain independently testable indexing, retrieval, and
context capabilities. The product keeps one default composition root and its
existing daemon/CLI behavior. More crates increase build-graph and API
maintenance cost, so extraction is limited to capabilities with independent
consumers and conformance suites. Provider implementations remain optional and
cannot make deterministic operation or offline builds depend on credentials,
network access, or model assets.
