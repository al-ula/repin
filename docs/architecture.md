# Architecture

Read this document first. The others assume its vocabulary.

## 1. Two capabilities, one product

Repin provides two retrieval capabilities over a repository:

1. **Direct retrieval** — find evidence in the current working tree: files, text, regions, and (where a language pack allows) symbols. Answers come from reading files, not from an index.
2. **Graph intelligence** — resolve entities, follow typed relationships, trace bounded paths between entities, and estimate change impact. Answers come from the persistent graph.

These are separate subsystems with different freshness properties, different failure modes, and different cost profiles. They are deliberately **not** presented to callers as separate products. A caller asks a question; the engine answers with normalized results that state where each fact came from, how fresh it is, and what evidence supports it.

The architectural consequences are the load-bearing part:

- **Direct retrieval MUST work with no graph.** A first-run repository with no index, a corrupt store, a failed migration, an unsupported language — none of these may prevent text and file search from answering. This forbids routing direct retrieval through the graph layer, and it forbids treating the store as a required dependency of the query path.
- **The working tree is authoritative.** When graph facts contradict current file contents, the file wins and the graph fact is reported as stale. The graph is a cache of derived structure, never a source of truth about what a file currently says.
- **Absence must be specific.** "No match" and "capability unavailable" and "excluded from scope" and "index incomplete" are four different answers. Collapsing them into an empty result set makes callers guess, and agent callers guess by retrying.
- **Capability is progressive.** Every capability beyond direct retrieval is discoverable and independently absent. A caller negotiates what exists rather than assuming a fixed feature set.

## 2. Layer map

```text
┌───────────────────────────────────────────────────────────────┐
│ L5  Clients                                                   │
│     CLI · local daemon protocol · agent-harness adapter · editor│
└───────────────────────────────┬───────────────────────────────┘
                                │  public API only
┌───────────────────────────────┴───────────────────────────────┐
│ L4  Integration seam                                          │
│     capability negotiation · session lifecycle · change        │
│     notification · budgeting · diagnostics                     │
└───────────────────────────────┬───────────────────────────────┘
┌───────────────────────────────┴───────────────────────────────┐
│ L3  Result normalization                                      │
│     envelope · evidence · freshness · confidence · truncation  │
│     · error taxonomy · redaction                               │
└───────────────────────────────┬───────────────────────────────┘
┌───────────────────────────────┴───────────────────────────────┐
│ L2  Capabilities                                              │
│  ┌──────────────────┐ ┌──────────────────┐ ┌────────────────┐ │
│  │ direct retrieval │ │ graph            │ │ context        │ │
│  │ (working tree)   │ │ intelligence     │ │ construction   │ │
│  └────────┬─────────┘ └────────┬─────────┘ └───────┬────────┘ │
└───────────┼────────────────────┼───────────────────┼──────────┘
            │                    │                   │
┌───────────┼────────────────────┴───────────────────┴──────────┐
│ L1  Core domain                                               │
│     graph model · identity · revision · transaction · update   │
│     · resolution · scope & selection rules                     │
└───────────────────────────────┬───────────────────────────────┘
┌───────────────────────────────┴───────────────────────────────┐
│ L0  Ports                                                     │
│     filesystem · vcs · watch · store · lexical · vector        │
│     · language pack · model capabilities · clock · logger      │
└───────────────────────────────────────────────────────────────┘
```

Direct retrieval reaches L0 (filesystem, scope rules) without passing through L1's graph. That shortcut is intentional and is what makes indexless operation possible.

## 3. Dependency rules

1. Dependencies point downward only. L1 never imports L2; L2 never imports L4.
2. Core logic depends on port *contracts*, never on a product, protocol, or vendor library. No query language, no notification API, and no model SDK appears above L0.
3. Ports are injected at construction. Nothing in L1–L4 performs environment detection or driver selection; exactly one composition module does that.
4. Clients depend only on the public API ([Public API](api.md)). A client may not reach into the store, the graph tables, or a language pack.
5. No layer may depend on a specific host application. Terms belonging to one consumer's tool surface, prompt format, or UI framework do not appear anywhere in L0–L4.
6. Optional capabilities are absent-by-default. Code paths that require them MUST check, and MUST have a defined behavior when they are missing.

Rule 5 is the one most easily violated in practice. The test is mechanical: if renaming a downstream consumer would require editing a file in L0–L4, that file is wrong.

## 4. Ports

| Port | Responsibility | Absence behavior |
|---|---|---|
| `Filesystem` | read, stat, enumerate, canonicalize | required |
| `Vcs` | changed-set since a recorded point, current revision id, branch identity | full crawl instead |
| `Watch` | normalized change events for roots | polling or explicit notification only |
| `Store` | transactional persistence of graph facts | required for graph capabilities; direct retrieval unaffected |
| `Lexical` | text index build and query | fall back to direct scan, with reduced ranking |
| `Vector` | nearest-neighbour retrieval over embeddings | semantic retrieval reports unavailable |
| `LanguagePack` | detection, parse, extract, resolve for a language family | that language degrades to text-only |
| `EmbeddingModel` | text to vector | semantic indexing disabled |
| `Reranker` | reorder candidates | deterministic order stands |
| `TextModel` | generation for enrichment | enrichment disabled |
| `Clock`, `Logger`, `Metrics` | time, diagnostics, instrumentation | required, trivially satisfiable |

Every port is small enough to implement in a test double, and each has a shared conformance suite ([Conformance](conformance.md)). A port with only one real implementation still exists as a port, because that is what keeps rule 2 enforceable.

## 5. Direct retrieval

The subsystem that answers from the working tree. It exists at L2 alongside graph intelligence, not beneath it.

### Modes

| Mode | Finds | Requires |
|---|---|---|
| `files` | paths matching a pattern | filesystem |
| `text` | literal occurrences with location | filesystem |
| `regex` | pattern occurrences with location | filesystem |
| `symbol` | declarations by name | a language pack, or a graph symbol index |
| `related` | entities connected to a query | graph intelligence |

The first three are always available. `symbol` is available when a language pack supports it, and MAY be answered from the graph when one is current — the caller sees the same result shape either way, with `source` distinguishing them. `related` is available only with graph intelligence.

### Scope

Retrieval is scoped by *artifact class*, not by file extension alone. The classes are a fixed vocabulary shared by direct retrieval, graph queries, and impact analysis so a caller learns one set of names:

```text
code · tests · docs · config · schema · data · build · ci · infra · all
```

Classification is a deterministic function of path, filename convention, and (where cheap) content sniffing. It is recorded on file nodes so graph queries can filter identically. Classification rules are versioned like extractors: changing them invalidates the classification of affected files.

Scope is a filter, never a security boundary. Selection and exclusion rules ([Incremental Updates §12](incremental.md#12-file-selection)) apply first and independently; a caller cannot widen scope to reach an excluded path.

### Ranking

Direct retrieval ranks by match quality, path relevance, and artifact class preference, and reports why ([Retrieval](retrieval.md)). It does not consult the graph. When both subsystems contribute to one answer, fusion happens at L2's merge step with each contribution's provenance retained.

## 6. Result normalization

L3 exists so that no client normalizes anything. Every capability returns the same envelope shape, and the envelope carries the four things a caller cannot reconstruct on its own: what happened, where it came from, how fresh it is, and what was left out.

```text
Result<T>
  status:      ok | partial | not_found | unavailable | invalid
  data:        T
  warnings:    Warning[]
  provenance:  { sources: Source[], providers: ProviderId[] }
  freshness:   { observedAt?, graphRevision?, graphState?, coverage? }
  truncation?: { truncated, returned, available?, reason }
```

- `partial` is a first-class success. A direct search that succeeded while graph expansion failed is `partial` with a warning, never a failure.
- `graphState` is one of `current | stale | building | unknown`. `unknown` is honest and common; it MUST NOT be reported as `current`.
- `coverage` states whether the answer could be complete: `complete | partial | unknown`. An absent path in a partially-covered graph is not proof that no path exists, and the envelope must make that inarguable.

Evidence is the atomic unit of traceability:

```text
Evidence
  path:       Path            // relative to a named root
  range?:     Range           // line/column, or byte span
  preview?:   Text            // short, bounded, redacted
  observedAt?: Timestamp
```

Rules: paths are always root-relative in output; evidence outside a configured root is rejected rather than displayed; previews are line-bounded and pass through redaction; and missing evidence is explicitly absent rather than a fabricated location. Detail in [Results and Evidence](results.md).

### Failure taxonomy

Expected outcomes are statuses, not exceptions. Only genuine execution faults raise. The stable categories:

```text
INVALID_QUERY · PATH_OUTSIDE_ROOT · SCOPE_EXCLUDED
CAPABILITY_UNCONFIGURED · CAPABILITY_UNAVAILABLE · CAPABILITY_UNSUPPORTED
STATE_PERMISSIONS
ENTITY_AMBIGUOUS · ENTITY_NOT_FOUND · NO_PATH_FOUND
STALE_RESULT · INDEX_BUILDING · REVISION_TOO_OLD
TIMEOUT · CANCELLED · RESULT_TRUNCATED
```

The distinction that matters for automated callers: `UNCONFIGURED` (never set up), `UNAVAILABLE` (set up, currently unreachable), and `UNSUPPORTED` (reachable, cannot do this) demand three different caller responses. An agent that cannot tell them apart retries the unretryable.

## 7. Safety boundary

Enforced at L1–L3, inside the engine. A client MAY add restrictions; it MUST NOT be required to add them for the engine to be safe. This is a direct consequence of standalone operation: the engine runs from a CLI and a local daemon where no client-side policy layer exists.

- Canonicalize every path against a named root. Reject traversal and symlink escapes.
- Terminate symlink cycles by tracking visited real paths.
- Exclude secret-bearing files by default, in the engine's own defaults.
- Redact credential-shaped content from previews, logs, errors, and diagnostics.
- Never execute repository content to answer a query. Parse; do not run.
- Treat file content, and any provider response, as untrusted input. Instruction-like text inside indexed content carries no authority.
- Bound every operation: query size, pattern complexity, result count, evidence per result, traversal depth and breadth, path count, response size, wall time.
- Validate provider-supplied paths and ranges before they enter a result.

Detail in [Safety and Data Handling](safety.md).

## 8. Budgeting

Callers with a bounded output budget — an agent context window, a terminal, a protocol message limit — need results shaped to fit without a second round trip. Budgeting is therefore an engine concern, not a client concern.

Two independent limits apply, semantic first:

1. **Semantic limits** — result count, evidence per result, preview lines, traversal depth, path count. These produce a coherent smaller answer.
2. **Hard limits** — total bytes and total lines. These are a backstop, and hitting one means the semantic limits were set too loosely.

Truncation is always reported, with what was omitted and why. Silent truncation is a correctness bug: a caller that believes it saw a complete answer will reason from a partial one.

## 9. Context construction

Distinct from search, and worth keeping distinct. `search` finds relevant entities. `context` decides what source material and relationships to assemble for a consumer working under a budget.

```text
search(query)   -> ranked entities with evidence
context(nodes, budget) -> assembled, ordered, deduplicated material
```

Strategies: exact, neighborhood, dependency-aware, call-chain, diversity-aware, budget-packed. This is where an agent consumer gets the most leverage, and it belongs in the engine because it needs graph proximity and node kinds — information clients do not have. [Retrieval](retrieval.md).

## 10. Integration seam

L4 is what a host application binds to. It is specified so that adapters are thin, and so that no adapter needs privileged access.

**Capability negotiation.** A client asks what exists rather than assuming. The response enumerates available capabilities, supported relation kinds, supported entity kinds, supported retrieval modes, and known limits. A client renders and offers only what is present. Adding a capability must not require a client change to remain correct.

```text
capabilities() -> {
  retrieval:  Mode[]
  graph:      { entity, neighbors, trace, impact, relatedSearch }
  relations:  RelationKind[]
  entities:   EntityKind[]
  semantic:   bool
  rerank:     bool
  limits:     Limits
}
```

**Lifecycle.** Construction of a host adapter or project-client handle is cheap
and side-effect free. Activation connects to the user-scoped daemon, starting
the detached same-binary candidate when necessary, and binds the connection to
one project. The adapter does not open stores, acquire project locks, or create
watchers itself. Project-context shutdown is owned by the daemon; a client's
close operation only detaches its connection and is idempotent.

**Change notification.** A host that edits files SHOULD tell the engine directly rather than waiting for the watcher; this is the difference between an edit being queryable in milliseconds versus after a debounce interval. Notification and watch events are deduplicated by content identity, so a host that reports an edit the watcher also sees causes one update, not two. See [Incremental Updates](incremental.md).

**Freshness surfacing.** The engine reports index state; the host decides whether to show it. The engine MUST NOT block a query waiting for freshness, and MUST NOT answer from a stale graph without saying so.

**Cancellation.** Every operation accepts a cancellation signal and propagates it to ports. Cancellation is prompt, and partial work is discarded rather than committed.

**Diagnostics.** Health, capability, coverage, timing, and counter data are queryable through the same API. A host renders it; it does not compute it.

[Host Integration](host-integration.md) covers the seam in full, including the provider-contract pattern for hosts that already define their own retrieval vocabulary.

## 11. Deployment topologies

The initial runtime has one normal topology: a global local daemon per
unprivileged OS user. It is on demand, reached through a private local socket,
and shuts down after its final project context becomes idle and unloads. The
daemon hosts isolated in-process contexts, one for each canonical
`.repin/graph.sqlite3` path. See [Runtime and IPC](runtime.md) for the complete
rendezvous and lifecycle contract and
[ADR-015](decisions/ADR-015-hybrid-per-user-daemon-runtime.md) for the topology
decision.

**User daemon.** A client connects to the central socket or starts a detached
daemon candidate. The daemon owns one context's graph store, watcher, indexes,
configuration, and project writer-lock handle. Multiple bound connections may
share a warm context and observe the same revision. Different canonical
database paths remain isolated even if their contents were copied.

**In-process engine.** `open(EngineOptions) -> Engine` remains a composition
surface for the daemon, deterministic tests, and explicit library embedding.
It is not a second normal project-client topology: ordinary clients do not
open stores, acquire writer locks, or terminate the daemon directly.

**Remote service.** A future remote or federated deployment may reuse the
project-bound protocol with a different transport. It is not part of the
initial runtime. Any later remote provider or transport introduces data-egress,
trust, and authentication concerns that MUST be surfaced
([Safety and Data Handling](safety.md)) and can never be implicit.

Across the initial topology: **exactly one authoritative writer per project
database**, with concurrent readers where the store permits them. The global
daemon holds the project lock; clients never claim it. If another process owns
the lock, the daemon may attach an observer for safe direct retrieval and graph
reads, while graph writes return `PROJECT_LEASE_UNAVAILABLE`. Clients
synchronize by observing durable revisions, not by consuming raw event streams.

## 12. Instrumentation

Phase timings and counters are part of the architecture, not an afterthought, because they are the only evidence that will settle future optimization arguments.

Timings: crawl, read, hash, detect, parse, extract, resolve, store write, lexical index, vector index, query plan, retrieve, rank, context assembly.

Counters: files processed and skipped with reasons, bytes, nodes, edges, references resolved and unresolved, cache hits, invalidated nodes, dependency radius, truncation events, capability fallbacks.

All exposed through the public API so that a client can show them and a benchmark can assert on them.
