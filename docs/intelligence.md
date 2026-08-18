# Optional Intelligence

Model-backed capabilities layered above the deterministic engine. Every one of them can be absent, and the engine must remain fully useful when they all are.

The framing that matters: these capabilities improve **recall on vague queries** and **ordering among close candidates**. They do not construct the graph, they do not resolve references, and they are never required to answer a question.

## 1. Capability-specific ports

No single broad "model provider" abstraction. Three separate ports, because they have different inputs, different costs, different failure modes, and are usually satisfied by different implementations.

```text
EmbeddingModel
  embed(texts: Text[]) -> Embedding[]
  identity() -> ModelIdentity
  dimensions() -> Count
  maxInputLength() -> Count

Reranker
  rerank(query: Text, candidates: RerankCandidate[]) -> RerankResult[]
  identity() -> ModelIdentity

TextModel
  generate(request: GenerateRequest) -> GenerateResult
  identity() -> ModelIdentity
```

```text
ModelIdentity
  provider: ProviderId
  model:    Text
  version?: Version
  location: local | remote | host_supplied
```

`provider` is the registered opaque alias described in [Results and Evidence
— Public provider identity](results.md#public-provider-identity). A model
adapter may keep vendor/account/endpoint details private; those details never
become a public provenance identifier merely because the port reports them.

A broad provider interface forces a caller to satisfy capabilities it does not need — configuring generation to get embeddings, for instance. Separate ports let a deployment enable embeddings with no generation, or reranking through a host's model with no other model access at all.

## 2. Provider sources

Any port may be satisfied from any source:

```text
local          in-process or local process
remote         network API
host_supplied  the embedding application's own model access
absent         capability disabled
```

`host_supplied` deserves emphasis. An application embedding Repin frequently already has model access, with its own credentials, quotas, retry behavior, and user consent. Requiring the engine to establish separate access duplicates configuration and produces surprising costs. Accepting the host's model implementation avoids all of it, and is the preferred arrangement when a host has one.

```text
host application
      │
      ├── engine: graph, retrieval
      └── model adapter ──> Reranker
```

## 3. Configuration

Per-capability, never a global switch.

```text
lexical:    enabled
graph:      enabled
semantic:   disabled
rerank:     enabled, provider: host_supplied
enrichment: disabled
```

A single flag cannot express "lexical yes, embeddings no, reranking through the host's model" — which is a completely ordinary configuration. Each capability declares its own state and provider independently.

Rules:

- Default for every model-backed capability is **disabled**.
- Enabling one never implicitly enables another.
- Every capability reports its state and provider identity in status output.
- Configuration holds no credentials; they come by reference from the environment or a host credential store.

## 4. Asynchrony

**A deterministic revision MUST NEVER block on model work.** This is the hardest rule in this document, and the most frequently violated in systems of this kind.

Wrong:

```text
file change ──> extract ──> embed ──> commit ──> revision
                              ↑
                    network latency, rate limits,
                    provider outage, cold model load
```

Right:

```text
                ┌── extract ──> commit ──> revision N      (immediately usable)
file change ────┤
                └── semantic queue ──> embed ──> semantic revision N-k
```

Consequences:

- An edit is queryable through deterministic channels immediately, regardless of embedding backlog.
- A provider outage degrades semantic recall. It does not stop indexing, and it does not stop queries.
- Semantic indexing may lag arbitrarily far behind without affecting correctness of deterministic answers.

Status reports both revisions and the backlog:

```text
IndexStatus
  revision:          Revision
  lexicalRevision?:  Revision
  lexicalPending?:   Count
  lexicalState?:     current | lagging | disabled | failing
  semanticRevision?: Revision
  semanticPending?:  Count
  semanticState?:    current | lagging | disabled | failing
```

Exposing the lag lets a caller reason about a surprising semantic result instead of concluding the index is broken.

## 5. Embedding cache

Re-embedding unchanged content is the dominant avoidable cost, both in time and in money for remote providers.

```text
semantic content ──> normalize ──> hash
                                    │
                     ┌── unchanged ─┴─ reuse
                     └── changed ─────  regenerate
```

Cache key components — all of them required:

```text
contentHash        normalized semantic content
providerIdentity   provider, model, version
dimensions         vector width
chunkingVersion    how content was split
renderingVersion   how the entity was turned into text
```

Omitting any one produces silent corruption rather than a miss. A key without model identity serves vectors from a different model; a key without chunking version serves vectors for different text. Both fail in ways that look like poor relevance rather than like a bug.

## 6. What gets embedded

The cache hash is only meaningful once the embedded text is defined precisely.

Each embedded unit is a **rendering** of one node:

```text
qualified name
signature or declaration form
documentation comment or summary
body, up to a configured limit
```

Rules:

- Rendering is deterministic and versioned. Changing it invalidates the cache, which is why `renderingVersion` is a key component.
- **Normalization before hashing** strips insignificant whitespace and comment-formatting differences, so reformatting a file does not invalidate its embeddings. Without this, a formatter run re-embeds the repository.
- Nodes exceeding the limit are **chunked deterministically**. Chunk boundaries must be a pure function of content: identical content yields identical chunks, or the cache is useless.
- Each chunk is one vector entry keyed by node id plus ordinal, individually addressable and collectively removable.
- `unstableId` nodes ([Graph Model §4](graph-model.md#4-identity)) are **never** cache keys. They are recreated on any edit to their file, so caching against them corrupts.
- Content excluded by selection rules is never embedded, and therefore never transmitted ([Safety and Data Handling §7](safety.md#7-data-egress)).

## 7. Semantic retrieval

Semantic hits enter the same merge as every other channel ([Retrieval §1](retrieval.md#1-channels)). They are not a separate result set, and not a fallback path.

- Filtered by metadata: root, language, node kind, artifact class. Unfiltered whole-repository similarity is rarely what a caller wants.
- Scored on a comparable scale so merge is meaningful. Raw distances from different metrics are not comparable and must be normalized.
- A hit resolving to a node that no longer exists is **dropped**, not returned. Semantic lag may surface stale content; it must never surface dangling references.
- Semantic-only results are marked, so a caller can see that a hit had no deterministic corroboration.

## 8. Enrichment

The most speculative capability, and correspondingly the most constrained. A model derives relations the parsers cannot:

```text
symbol   ──> concept
module   ──> responsibility
document ──> topic
function ──> behavior summary
```

Non-negotiable constraints:

1. **Every enriched fact carries `derivation: inferred`** ([Graph Model §3](graph-model.md#3-provenance)), forever, at every layer.
2. **Stored separately** from deterministic facts, and independently deletable. Discarding all enrichment must leave a valid graph — invariant 8 in [Graph Model §8](graph-model.md#8-graph-invariants).
3. **Disposable and rebuildable.** Enrichment is never required to migrate ([Storage §8](storage.md#8-migration)) and never load-bearing for another fact.
4. **A deterministic fact never depends on an inferred one.** Not in resolution, not in ranking weights, not in traversal.
5. **Filterable out entirely.** `derivation` filters let a caller work with deterministic facts only.
6. **Deferred until deterministic retrieval is mature.** Enrichment layered over weak deterministic retrieval hides the weakness rather than fixing it, and makes the underlying quality unmeasurable.

Point 6 is a sequencing judgment, not a technical constraint. Enrichment is appealing precisely when deterministic retrieval is disappointing, which is exactly when it does the most harm.

## 9. Failure behavior

| Capability | Fails | Result |
|---|---|---|
| embedding | queue retains work, backoff, report `failing` | deterministic unaffected |
| rerank | deterministic order stands | `ok`, not `partial` |
| generation | enrichment paused | deterministic unaffected |

Rules:

- Every model call is deadline-bounded. No model call blocks a query indefinitely.
- Failures are reported in status, not raised to callers whose queries succeeded on deterministic channels.
- Retries use bounded backoff. A failing provider must not become a hot loop against a rate limit.
- **Failure of an optional capability is never a failed query.** A caller that asked a question the deterministic engine could answer gets `ok`. Marking it `partial` teaches callers to distrust successful answers.

## 10. Cost and consent

Model capabilities cost money and may transmit repository content. Both must be visible.

- Remote providers are opt-in per capability, never enabled as a side effect.
- Endpoint and model identity are reported in status, so an operator can discover where content goes without reading configuration.
- Remote providers require a trusted project.
- Embedding volume is measurable before it is incurred: how many entities, how many cache misses, how much content. A caller enabling semantic indexing on a large repository should be able to learn the cost first rather than discover it from an invoice.
- Cache hit rate is reported. It is the primary lever on cost, and an unexpectedly low rate usually means a key component is changing when it should not.
