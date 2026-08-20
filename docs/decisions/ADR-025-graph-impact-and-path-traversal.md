# ADR-025: Graph Impact Analysis and Dependency Path Traversal

```text
Status: accepted contract and capability decision
Date: 2026-08-21
Decision type: graph traversal, blast radius impact analysis, and dependency path tracing
Builds on: ADR-005, ADR-015, ADR-016, ADR-017, ADR-023
```

## Decision

Repin adopts dedicated, first-class graph blast-radius analysis and dependency path tracing operations across its runtime, protocol, daemon, and CLI interfaces:

1. **`impact` (Blast Radius Analysis):**
   - Transitive upstream traversal across incoming edges (`Calls`, `Contains`, `References`, `Imports`, `Extends`) using bounded Breadth-First Search (BFS).
   - Computes all affected symbols, files, modules, and tests downstream from a proposed modification.
   - Enforces a default `max_depth` of 3 (configurable via `--max-depth`) and cycle deduplication via `HashSet<NodeId>`.

2. **`path` (Shortest Dependency Path Tracing):**
   - Computes the shortest directed dependency/call path connecting a source node (`from`) and destination node (`to`).
   - Returns a structured sequence of intermediate nodes and connecting edge relationship kinds.
   - Enforces a default `max_depth` of 5 (configurable via `--max-depth`) and path-level cycle detection.

3. **Wire Protocol & IPC Contracts:**
   - Extend `IpcRequest` with `Impact { name_or_id: String, max_depth: Option<usize> }` and `Path { from: String, to: String, max_depth: Option<usize> }`.
   - Extend `IpcResponse` with `ImpactResult(ResultEnvelope<serde_json::Value>)` and `PathResult(ResultEnvelope<serde_json::Value>)`.

4. **Deterministic Formatting & CLI Surface:**
   - Expose `repin impact <TARGET> [--max-depth <N>] [--json]` and `repin path <FROM> <TO> [--max-depth <N>] [--json]`.
   - Provide human-readable, colorized terminal hierarchy summaries grouped by depth level, alongside machine-readable JSON envelopes.

All operations adhere to Repin's core invariants:
- **Read-only Snapshot Consistency:** Traversals execute against deterministic, point-in-time `ReadView` handles with zero write amplification.
- **Fail-Closed & Bounded:** Queries on nonexistent entities return structured not-found envelopes without panic; traversal depth is strictly capped.
- **Deterministic Ordering:** Neighbor edges and paths are sorted by deterministic `(NodeId, EdgeKind)` keys before presentation.

## Rationale

When planning refactors, investigating regressions, or conducting AI-assisted code generation, developers and autonomous coding agents require immediate understanding of:
1. What upstream components will break if a function, struct, or interface changes (`impact`).
2. Exactly how two seemingly distant components interact or depend on each other (`path`).

While Repin previously utilized blast radius calculations internally during review context assembly (ADR-016, ADR-017), exposing dedicated `impact` and `path` CLI and IPC endpoints eliminates the need for manual caller hopping or external graph visualizers.

## Consequences

- `repin-retrieval` defines structured `ImpactData`, `ImpactItem`, `PathTraceData`, and `PathSegment` models.
- `repin-protocol` adds `Impact` and `Path` request/response variants to the IPC envelope.
- `repin-runtime` and `repin-daemon` expose `lookup_impact` and `trace_paths` methods.
- `repin-cli` introduces `impact` and `path` subcommands.
- Benchmark and evaluation suites (`scripts/benchmark_suite.py`) measure and track impact and path query latencies against CodeGraph and Graphify.

## Required Implementation Validation

1. **Cycle Safety:** Recursive call cycles (e.g. `A -> B -> A`) must terminate cleanly without infinite loops or duplicate path nodes.
2. **Depth Invariance:** Traversals must never explore deeper than the configured `max_depth`.
3. **Determinism:** Identical graphs must yield identical impact ordering and shortest path choices across multiple invocations.
4. **Disconnected Nodes:** Path queries between unreachable symbols must return empty traces gracefully without errors.
