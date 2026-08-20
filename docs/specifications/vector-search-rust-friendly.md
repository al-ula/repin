# Specification: Exact Rust vector search baseline

```text
Status: accepted future subsystem specification backing ADR-012
Milestone: I5 — Embeddings
Scope: optional semantic retrieval
Primary recommendation: exact brute-force search in Rust
```

## 1. Specification

Semantic retrieval starts at milestone I5 with an exact, pure-Rust vector implementation rather
than a native approximate-nearest-neighbour (ANN) library.

The implementation stores embedding entries and their searchable metadata in
the SQLite-backed project state. The adapter applies metadata filters, streams
the remaining vectors row by row as `Vec<f32>`, computes the configured
distance in Rust, retains a bounded top-k heap, and returns a stable,
score-ordered result set. It does not materialize the entire filtered candidate
set in memory.

This specification applies to the vector channel only. It preserves the
accepted SQLite + FTS5 profile for authoritative graph and lexical storage.

## 2. Architectural rationale

The exact baseline provides the minimal Rust and recovery surface:

- no C or C++ binding and no native ANN build step;
- no opaque ANN file format or graph-topology migration;
- deletion is an ordinary keyed removal and is easy to verify after reopen;
- SQLite performs required metadata filtering;
- exact recall provides a reference oracle for later ANN evaluation;
- deterministic score and `VectorKey` tie-breaking are straightforward; and
- the vector adapter remains optional and independently revisioned.

The trade-off is query cost: exact search is `O(number_of_candidates ×
dimension)`. That cost is measured during milestone I5 benchmarks.

## 3. Data and query shape

Each vector entry adheres to the `Vector` port contract:

```text
VectorEntry
  key:       node id plus chunk ordinal
  embedding: fixed-dimension f32 values
  metadata:  root, language?, node kind?, artifact class?

VectorQuery
  embedding: fixed-dimension f32 values
  filters:   metadata filters
  limit:     result count
```

The adapter executes the query pipeline:

1. reject dimension or metric mismatches;
2. select only entries matching the requested metadata filters;
3. compute cosine, inner-product, or Euclidean distance according to the
   fixed index configuration;
4. retain the best bounded candidate set;
5. sort by semantic score and then stable `VectorKey`; and
6. resolve each hit against the authoritative graph before returning it.

The vector revision remains asynchronous. A deterministic graph revision must
not wait for embedding generation or vector writes. A provider outage or
embedding backlog reduces semantic freshness only; graph, lexical, and direct
retrieval remain fully usable.

## 4. Persistence and recovery

Embedding work follows the derived-index protocol:

1. commit authoritative graph facts and revision in SQLite;
2. record reconstructable embedding work or source state;
3. generate embeddings and apply vector updates in subsequent transactions;
4. persist the vector revision and acknowledge completion; and
5. repair or rebuild after interruption.

An acknowledged removal must remain absent after reopen. The graph is always
authoritative, and stale vector hits for deleted graph entities are dropped.

## 5. ANN escalation path

If exact search fails the fixed-corpus latency or memory budget during milestone I5, evaluate a
pure-Rust ANN adapter against the unchanged `Vector` contract.

`hnsw_rs` is the leading ANN fallback: its documentation describes dump/reload
support and filtering during search, and it is published under MIT/Apache-2.0
terms. Its documented API does not expose Repin's required deletion operation,
so the adapter would need a tested tombstone/rebuild policy or a full rebuild
on deletion.

The older `instant-distance` crate is a less attractive fallback because its
documented surface is narrower and its latest documented release is older.

USearch is excluded from the primary profile to avoid external native build and distribution complexities.

## 6. I5 acceptance criteria

The exact implementation is validated for semantic retrieval when it demonstrates:

- dimension and metric enforcement;
- filtered search over the required metadata fields;
- acknowledged deletion that survives reopen;
- crash/interruption repair without graph rollback;
- deterministic ordering for equal scores;
- bounded memory and deadline behavior; and
- a measured precision-at-N and latency baseline.

## 7. Non-decisions

This specification does not select an embedding model or provider, enable semantic
retrieval by default, or change the I5 sequencing in
[ADR-007](../decisions/ADR-007-optional-capability-sequencing.md). It does
not modify the persistence selections in [ADR-009](../decisions/ADR-009-sqlite-fts5-initial-profile.md).
