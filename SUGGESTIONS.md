# Architectural & Feature Improvement Suggestions for Repin

This document outlines prioritized improvement proposals for the **Repin** repository intelligence engine, informed by empirical comparative benchmarks against [`codegraph`](file:///home/seer/.local/bin/codegraph) (v1.5.0) and [`graphify`](file:///home/seer/.local/bin/graphify) (v0.9.28) using [`scripts/benchmark_suite.py`](file:///home/seer/Projects/repin/scripts/benchmark_suite.py).

---

## Executive Summary & Benchmark Takeaways

In side-by-side evaluations across the `repin` codebase (77 Rust source files, 10 crates):
* ⚡ **Indexing Speed:** `repin` is the fastest cold indexer (**~0.62 s** vs `codegraph`'s **0.87 s** and `graphify`'s **1.74 s**).
* ⚡ **Query Latency:** `repin` answers hybrid search and context queries in **20–45 ms** (vs 140–280 ms in alternative tools).
* 🔍 **Opportunities:** Key areas for growth are **verbatim source context extraction**, **graph centrality rank signals**, **storage footprint compaction**, and **native agent protocol (MCP) support**.

```
┌────────────────────────────────────────────────────────────────────────┐
│                          IMPROVEMENT ROADMAP                           │
├──────────────────────────────┬──────────────────────────┬──────────────┤
│ High Priority (P1)           │ Medium Priority (P2)     │ Future (P3)  │
├──────────────────────────────┼──────────────────────────┼──────────────┤
│ 1. Verbatim Code Context     │ 4. Multi-Hop BFS Graph   │ 6. Native    │
│ 2. Centrality Rank Boost     │ 5. Expose `impact/path`  │    MCP Server│
│ 3. SQLite Storage Compaction │                          │ 7. Visual D3 │
│                              │                          │    Export    │
└──────────────────────────────┴──────────────────────────┴──────────────┘
```

---

## 1. High Priority Improvements (P1)

### 1.1 Verbatim Line-Numbered Source in Context Packing
* **Location:** [`crates/repin-engine/src/context.rs`](file:///home/seer/Projects/repin/crates/repin-engine/src/context.rs)
* **Current Behavior:** [`ContextBuilder::assemble_neighborhood`](file:///home/seer/Projects/repin/crates/repin-engine/src/context.rs#L26-L61) only emits text metadata headers (`Symbol: Engine (struct)\nFile: ...\nLine Range: ...\nAttributes: ...`).
* **Proposed Enhancement:**
  * When packing a node within the byte budget, load the actual source lines from the working tree via `repin-fs` or line index cache.
  * Emit formatted 1-indexed code slices (e.g. `32: pub struct Engine { ... }`).
  * Include immediate caller and dependency counts (blast radius) at the top of the context block.
* **Impact:** Eliminates the need for coding agents (Gemini, Claude, Cursor) to make secondary `read_file` round-trips after receiving context.

### 1.2 Graph Degree Centrality / Hub Weighting in Rank Fusion
* **Location:** [`crates/repin-engine/src/ranking.rs`](file:///home/seer/Projects/repin/crates/repin-engine/src/ranking.rs)
* **Current Behavior:** [`DeterministicRanker`](file:///home/seer/Projects/repin/crates/repin-engine/src/ranking.rs#L27-L125) scores candidates based on exact name match, prefix match, substring match, path proximity, FTS5 BM25 lexical score, and artifact class.
* **Proposed Enhancement:**
  * Ingest node in-degree (number of incoming references/calls) as a normalized centrality signal:
    $$\text{score}_{\text{centrality}} = \min\left(1.0, \frac{\text{in\_degree}}{\text{max\_degree}}\right) \times 0.15$$
  * Distinguish high-centrality architectural hubs (e.g. `Engine`, `StoreError`, `DaemonClient`) from low-centrality local helpers and temporary identifiers sharing the same name.
* **Impact:** Significantly boosts Precision@1 and Mean Reciprocal Rank (MRR) for architectural queries.

### 1.3 Storage Footprint Compaction & Post-Index WAL Checkpoint
* **Location:** [`crates/repin-store-sqlite/src/schema.rs`](file:///home/seer/Projects/repin/crates/repin-store-sqlite/src/schema.rs) and [`crates/repin-store-sqlite/src/store.rs`](file:///home/seer/Projects/repin/crates/repin-store-sqlite/src/store.rs)
* **Analysis & Key Findings:**
  * **Markdown Accounts for 62.6% of Nodes:** `repin` indexed 136 files (77 Rust + 59 Markdown docs/ADRs), producing **879 markdown nodes** and **524 rust nodes** plus full FTS5 search indexes. In contrast, `codegraph` indexed only 77 Rust files (0 markdown).
  * **Uncheckpointed WAL:** The `.repin` directory appeared as **6.8 MB** because `repin index` left an uncheckpointed 4.44 MB `graph.sqlite3-wal` file. The actual database is only **2.68 MB**.
  * Running `PRAGMA wal_checkpoint(TRUNCATE);` immediately drops the total disk size to **2.73 MB** (which is smaller than `codegraph`'s 3.32 MB despite indexing nearly 2x the files).
* **Proposed Enhancement:**
  * Auto-execute `PRAGMA wal_checkpoint(TRUNCATE);` at the end of `index` and `update` batch commits in [`crates/repin-store-sqlite/src/store.rs`](file:///home/seer/Projects/repin/crates/repin-store-sqlite/src/store.rs).
  * Intern repeated string columns (`root`, `path`, `producer`, `producer_version`) in `node_claims` and `edge_claims` using integer foreign keys into a `paths` / `producers` table.
  * Compress `provenance_json` and `attributes_json` or store only non-default fields.
* **Impact:** Immediately drops on-disk index size to **<2.5 MB** even with full multi-modal Markdown + Rust + FTS5 indexing.

---

## 2. Medium Priority Improvements (P2)

### 2.1 True Multi-Hop BFS Traversal with Depth Bounds
* **Location:** [`crates/repin-engine/src/traversal.rs`](file:///home/seer/Projects/repin/crates/repin-engine/src/traversal.rs)
* **Current Behavior:** [`GraphTraversal::lookup_neighbors`](file:///home/seer/Projects/repin/crates/repin-engine/src/traversal.rs#L43-L47) accepts `_max_depth: usize` but ignores it, performing only a single-hop query.
* **Proposed Enhancement:**
  * Implement breadth-first search (BFS) up to `max_depth` (default: 1, configurable up to 5).
  * Maintain a `visited: HashSet<NodeId>` to guard against cyclic references.
  * Support edge-kind filtering (e.g. `--kinds Calls,References` vs `--kinds Contains`).
* **Impact:** Enables deep call-tree tracing and dependency hierarchy exploration via `repin neighbors <SYMBOL> --max-depth 3`.

### 2.2 Expose `impact` and `path` CLI & Protocol Subcommands
* **Location:** [`crates/repin-cli/src/commands`](file:///home/seer/Projects/repin/crates/repin-cli/src/commands) and [`crates/repin-protocol/src/ipc.rs`](file:///home/seer/Projects/repin/crates/repin-protocol/src/ipc.rs)
* **Current Behavior:** [`GraphTraversal::impact_analysis`](file:///home/seer/Projects/repin/crates/repin-engine/src/traversal.rs#L116) and [`GraphTraversal::trace_paths`](file:///home/seer/Projects/repin/crates/repin-engine/src/traversal.rs#L81) are implemented in the engine but not exposed via CLI commands.
* **Proposed Enhancement:**
  * Add `repin impact <SYMBOL_OR_FILE>`: Displays affected downstream symbols, modules, and tests (blast radius).
  * Add `repin path <FROM> <TO>`: Computes and displays the shortest dependency / call chain between two symbols.
* **Impact:** Gives developers and autonomous agents instant impact analysis during refactors.

---

## 3. Ecosystem & Workflow Improvements (P3)

### 3.1 Model Context Protocol (MCP) Server Adapter
* **Location:** `crates/repin-mcp/` or `repin mcp` subcommand
* **Proposed Enhancement:**
  * Expose an MCP server over JSON-RPC (stdio or SSE) implementing standard tools:
    * `repin_search(query, mode, limit)`
    * `repin_context(query, budget_tokens)`
    * `repin_impact(symbol, max_depth)`
    * `repin_inspect(path)`
* **Impact:** Allows seamless 1-click integration with Claude Code, Cursor, Antigravity, Gemini CLI, and OpenCode without custom shell scripts.

### 3.2 Visual Architecture & Call-Flow Export
* **Location:** `crates/repin-cli/src/commands/export.rs`
* **Proposed Enhancement:**
  * Add `repin export --format d3-html --out graph.html`: Emits an interactive, collapsible D3 graph view of the repository.
  * Add `repin export --format mermaid`: Emits Mermaid architecture diagrams for documentation and GitHub pull requests.
* **Impact:** Bridges the gap between code intelligence and human-readable architecture documentation.

---

## Verification & Benchmarking Workflow

All improvements can be verified against the baseline using the automated suite:

```bash
# 1. Run baseline comparison
python3 scripts/benchmark_suite.py --json-out /tmp/baseline.json

# 2. Implement improvement & build release
cargo build --release --workspace

# 3. Re-run benchmark to verify speed, size, and token efficiency
python3 scripts/benchmark_suite.py --json-out /tmp/after.json
```
