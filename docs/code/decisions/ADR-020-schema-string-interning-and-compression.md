# ADR-020: Schema string interning and JSON attribute compression

```text
Status: accepted contract and capability decision
Date: 2026-08-20
Decision type: storage normalization, disk footprint compaction, and index efficiency
Builds on: ADR-009, ADR-019
```

## Decision

Repin adopts normalized string dictionary pooling and compact JSON attribute/provenance serialization within the SQLite graph storage layer (`repin-store-sqlite`):

1. **String and Fact Owner Normalization**:
   - Introduce a `string_pool` table to deduplicate paths, repository roots, language names, and extractor strings.
   - Introduce a `fact_owners` table to normalize the composite `(root, path, producer, producer_version)` fact ownership tuples into a single integer surrogate key `owner_id`.
   - Update `node_claims`, `edge_claims`, `unresolved_refs`, `skips`, and `diagnostics` to reference `owner_id` (INTEGER) instead of storing redundant 4-tuple text columns.
   - Replace the ~200+ byte composite string primary keys on claim tables with compact `(node_id, owner_id)` and `(edge_id, owner_id)` composite keys.

2. **In-Memory Interner Caching**:
   - `SqliteTransaction` and `SqliteReadView` maintain in-memory `HashMap` interner caches during batch ingestion and reads.
   - Common strings and fact owners are resolved in-memory with zero per-row query overhead, and uncached entities are batch-inserted via `INSERT OR IGNORE`.

3. **Compact JSON & Attribute Payload Optimization**:
   - AST nodes and edges with empty attribute maps (`{}`) store `NULL` or 0-byte blobs rather than repeated text JSON strings.
   - Provenance records omitting custom ranges or using default extraction parameters are stored compactly.

4. **Port Contract Invariance**:
   - The public domain models (`Node`, `Edge`, `NodeClaim`, `EdgeClaim`, `FactOwner`, `Provenance`) and port traits (`Store`, `Transaction`, `ReadView`) in `repin-core` remain unchanged. Physical normalization is strictly encapsulated inside the SQLite storage adapter.

## Rationale

Detailed storage analysis of the Repin graph database revealed that repeated strings (`root`, `path`, `producer`, `producer_version`) and boilerplate text JSON blobs (`"{}"`, standard provenance) accounted for over 45% of the database table and index page allocations. Storing 4 text columns in the primary key B-trees bloated primary and secondary index pages.

By normalizing fact owners into an integer surrogate key and interning strings in a shared pool:
- Primary key index size drops from ~120–240 bytes per row to 40 bytes per row (`node_id [32B]` + `owner_id [8B]`).
- Base database disk footprint decreases by ~30–45% (<2.0 MB on medium repos).
- Cache locality and B-tree page density in SQLite are significantly improved.

## Consequences

- Schema DDL introduces `string_pool` and `fact_owners` tables with foreign key constraints.
- `SqliteTransaction` utilizes `StringInterner` and `OwnerInterner` to resolve surrogate IDs during `put_nodes`, `put_edges`, `put_unresolved`, `put_skips`, and `put_diagnostics`.
- `SqliteReadView` joins with `fact_owners` and `string_pool` to materialize full domain models transparently.
- Deletion operations (`remove_claims`, `remove_by_file`, `remove_node_claims`) operate on `owner_id` or `path_id` integer indices.
- Total database disk size is reduced while retaining complete snapshot isolation and deterministic query guarantees.

## Required implementation validation

1. `cargo test -p repin-store-sqlite` verifies insert, query, update, and removal across all fact and claim types with full string and owner reconstruction.
2. `cargo test -p repin-conformance` passes all conformance suites (I0/I1 replay, isolation, and transaction rollback).
3. Benchmark suite (`scripts/benchmark_suite.py`) confirms database size reduction and sub-25ms query response times.

## Reopen triggers

Reopen this decision if dictionary join overhead measurably degrades read latency on large graphs (>100k nodes) compared to denormalized storage.
