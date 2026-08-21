# Summary

# Part I: Usage

- [Usage Guide](usage/index.md)
  - [Quick Start](usage/quickstart.md)
  - [CLI Reference](usage/cli.md)
  - [Configuration](usage/configuration.md)
  - [Agent and Host Integration](usage/integration.md)
  - [Troubleshooting](usage/troubleshooting.md)

# Part II: Code and Architecture

- [Code and Architecture](code/index.md)
- [Introduction](code/introduction.md)

## Architecture Foundations

- [Architecture and Layers](code/architecture.md)
- [Safety and Data Handling](code/safety.md)
- [Results and Evidence Model](code/results.md)

## Core Domain and Data Model

- [Graph Model and Invariants](code/graph-model.md)
- [Extraction and Language Packs](code/extraction.md)
- [Incremental Updates and Convergence](code/incremental.md)
- [Storage, Transactions and Persistence](code/storage.md)

## Query and Integration Surfaces

- [Retrieval, Ranking and Context](code/retrieval.md)
- [Public API Specification](code/api.md)
- [Runtime, IPC and Daemon Architecture](code/runtime.md)
- [Host Integration Seam](code/host-integration.md)
- [Optional Intelligence](code/intelligence.md)
- [Embedded RAG Proof](code/embedded-rag.md)

## Quality, Conformance and Implementation

- [Conformance and Verification](code/conformance.md)
- [Reusable-library extraction baseline](code/benchmarks/library-extraction-baseline.md)
- [Technology Selections and Implementation Profile](code/technology-candidates.md)
- [Implementation Roadmap and Milestones](code/roadmap.md)

## Architectural Decision Records

- [Decision Ledger](code/decisions/index.md)
  - [ADR-001 — Linux PoC scope](code/decisions/ADR-001-linux-poc-scope.md)
  - [ADR-002 — Synchronous core](code/decisions/ADR-002-synchronous-core.md)
  - [ADR-003 — Capability-relative filesystem](code/decisions/ADR-003-capability-relative-filesystem.md)
  - [ADR-004 — Update and hash protocol](code/decisions/ADR-004-update-hash-protocol.md)
  - [ADR-005 — Deterministic search contracts](code/decisions/ADR-005-deterministic-search-contracts.md)
  - [ADR-006 — Extraction and ranges](code/decisions/ADR-006-extraction-and-ranges.md)
  - [ADR-007 — Optional capability sequencing](code/decisions/ADR-007-optional-capability-sequencing.md)
  - [ADR-008 — Provisional quality and content policy](code/decisions/ADR-008-provisional-quality-and-content-policy.md)
  - [ADR-009 — SQLite + FTS5 initial profile](code/decisions/ADR-009-sqlite-fts5-initial-profile.md)
  - [ADR-010 — `regex` direct-search adapter](code/decisions/ADR-010-regex-direct-search.md)
  - [ADR-011 — Bounded Git subprocess](code/decisions/ADR-011-bounded-git-subprocess.md)
  - [ADR-012 — Exact Rust vector baseline](code/decisions/ADR-012-exact-rust-vector-baseline.md)
  - [ADR-013 — Native parsers with Tree-sitter fallback](code/decisions/ADR-013-native-parser-tree-sitter-fallback.md)
  - [ADR-014 — Sparse-checkpoint line index](code/decisions/ADR-014-sparse-checkpoint-line-index.md)
  - [ADR-015 — Per-user daemon with in-process engine](code/decisions/ADR-015-hybrid-per-user-daemon-runtime.md)
  - [ADR-016 — Agent inspection and review context](code/decisions/ADR-016-agent-inspection-and-review-context.md)
  - [ADR-017 — Verbatim context and blast radius](code/decisions/ADR-017-verbatim-context-and-blast-radius.md)
  - [ADR-018 — Graph degree centrality in ranking](code/decisions/ADR-018-graph-degree-centrality-rank-fusion.md)
  - [ADR-019 — SQLite WAL checkpoint and compaction](code/decisions/ADR-019-sqlite-wal-checkpoint-and-compaction.md)
  - [ADR-020 — Schema string interning and JSON compression](code/decisions/ADR-020-schema-string-interning-and-compression.md)
  - [ADR-021 — Per-project configuration and merge protocol](code/decisions/ADR-021-per-project-configuration.md)
  - [ADR-022 — Multi-tier model provider architecture](code/decisions/ADR-022-multi-tier-model-providers.md)
  - [ADR-023 — Reusable capability crates and runtime facade](code/decisions/ADR-023-reusable-library-crates.md)
  - [ADR-024 — Compatibility versioning and conservative state replacement](code/decisions/ADR-024-compatibility-versioning.md)
  - [ADR-025 — Graph impact analysis and dependency path traversal](code/decisions/ADR-025-graph-impact-and-path-traversal.md)
  - [ADR-026 — Daemon-mediated state lifecycle and database identity](code/decisions/ADR-026-daemon-mediated-state-lifecycle.md)

## Subsystem Specifications

- [Subsystem Specification Index](code/specifications/index.md)
  - [Per-Project Configuration (`config.toml`)](code/specifications/project-configuration.md)
  - [Sparse-checkpoint line index](code/specifications/sparse-line-index.md)
  - [Language-native parsers with Tree-sitter fallback](code/specifications/native-parsers-tree-sitter-fallback.md)
  - [Exact Rust vector search baseline](code/specifications/vector-search-rust-friendly.md)
  - [Agent inspection and review context](code/specifications/agent-inspection-and-review-context.md)
  - [Multi-tier model provider architecture](code/specifications/multi-tier-model-providers.md)

## Concluded Research and Trade Studies

- [Research Index](code/research/index.md)
  - [redb + Tantivy versus SQLite + FTS5](code/research/redb-tantivy-vs-sqlite.md)
  - [libSQL embedded-local](code/research/libsql-embedded-local.md)
