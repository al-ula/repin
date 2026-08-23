# Summary

- [Code and Architecture](index.md)
- [Introduction](introduction.md)

## Architecture Foundations

- [Architecture and Layers](architecture.md)
- [Safety and Data Handling](safety.md)
- [Results and Evidence Model](results.md)

## Core Domain and Data Model

- [Graph Model and Invariants](graph-model.md)
- [Extraction and Language Packs](extraction.md)
- [Incremental Updates and Convergence](incremental.md)
- [Storage, Transactions and Persistence](storage.md)

## Query and Integration Surfaces

- [Retrieval, Ranking and Context](retrieval.md)
- [Public API Specification](api.md)
- [Runtime, IPC and Daemon Architecture](runtime.md)
- [Host Integration Seam](host-integration.md)
- [Optional Intelligence](intelligence.md)
- [Embedded RAG Proof](embedded-rag.md)

## Quality, Conformance and Implementation

- [Conformance and Verification](conformance.md)
- [Reusable-library extraction baseline](benchmarks/library-extraction-baseline.md)
- [Technology Selections and Implementation Profile](technology-candidates.md)
- [Implementation Roadmap and Milestones](roadmap.md)

## Architectural Decision Records

- [Decision Ledger](decisions/index.md)
  - [ADR-001 — Linux PoC scope](decisions/ADR-001-linux-poc-scope.md)
  - [ADR-002 — Synchronous core](decisions/ADR-002-synchronous-core.md)
  - [ADR-003 — Capability-relative filesystem](decisions/ADR-003-capability-relative-filesystem.md)
  - [ADR-004 — Update and hash protocol](decisions/ADR-004-update-hash-protocol.md)
  - [ADR-005 — Deterministic search contracts](decisions/ADR-005-deterministic-search-contracts.md)
  - [ADR-006 — Extraction and ranges](decisions/ADR-006-extraction-and-ranges.md)
  - [ADR-007 — Optional capability sequencing](decisions/ADR-007-optional-capability-sequencing.md)
  - [ADR-008 — Provisional quality and content policy](decisions/ADR-008-provisional-quality-and-content-policy.md)
  - [ADR-009 — SQLite + FTS5 initial profile](decisions/ADR-009-sqlite-fts5-initial-profile.md)
  - [ADR-010 — `regex` direct-search adapter](decisions/ADR-010-regex-direct-search.md)
  - [ADR-011 — Bounded Git subprocess](decisions/ADR-011-bounded-git-subprocess.md)
  - [ADR-012 — Exact Rust vector baseline](decisions/ADR-012-exact-rust-vector-baseline.md)
  - [ADR-013 — Native parsers with Tree-sitter fallback](decisions/ADR-013-native-parser-tree-sitter-fallback.md)
  - [ADR-014 — Sparse-checkpoint line index](decisions/ADR-014-sparse-checkpoint-line-index.md)
  - [ADR-015 — Per-user daemon with in-process engine](decisions/ADR-015-hybrid-per-user-daemon-runtime.md)
  - [ADR-016 — Agent inspection and review context](decisions/ADR-016-agent-inspection-and-review-context.md)
  - [ADR-017 — Verbatim context and blast radius](decisions/ADR-017-verbatim-context-and-blast-radius.md)
  - [ADR-018 — Graph degree centrality in ranking](decisions/ADR-018-graph-degree-centrality-rank-fusion.md)
  - [ADR-019 — SQLite WAL checkpoint and compaction](decisions/ADR-019-sqlite-wal-checkpoint-and-compaction.md)
  - [ADR-020 — Schema string interning and JSON compression](decisions/ADR-020-schema-string-interning-and-compression.md)
  - [ADR-021 — Per-project configuration and merge protocol](decisions/ADR-021-per-project-configuration.md)
  - [ADR-022 — Multi-tier model provider architecture](decisions/ADR-022-multi-tier-model-providers.md)
  - [ADR-023 — Reusable capability crates and runtime facade](decisions/ADR-023-reusable-library-crates.md)
  - [ADR-024 — Compatibility versioning and conservative state replacement](decisions/ADR-024-compatibility-versioning.md)
  - [ADR-025 — Graph impact analysis and dependency path traversal](decisions/ADR-025-graph-impact-and-path-traversal.md)
  - [ADR-026 — Daemon-mediated state lifecycle and database identity](decisions/ADR-026-daemon-mediated-state-lifecycle.md)
  - [ADR-027 — CLI flag overrides](decisions/ADR-027-cli-override-flags.md)
  - [ADR-028 — Centralized product layout](decisions/ADR-028-centralized-path-layout.md)
  - [ADR-029 — Consolidated crate topology](decisions/ADR-029-consolidated-crate-topology.md)

## Subsystem Specifications

- [Subsystem Specification Index](specifications/index.md)
  - [Per-Project Configuration (`config.toml`)](specifications/project-configuration.md)
  - [Sparse-checkpoint line index](specifications/sparse-line-index.md)
  - [Language-native parsers with Tree-sitter fallback](specifications/native-parsers-tree-sitter-fallback.md)
  - [Exact Rust vector search baseline](specifications/vector-search-rust-friendly.md)
  - [Agent inspection and review context](specifications/agent-inspection-and-review-context.md)
  - [Multi-tier model provider architecture](specifications/multi-tier-model-providers.md)

## Concluded Research and Trade Studies

- [Research Index](research/index.md)
  - [redb + Tantivy versus SQLite + FTS5](research/redb-tantivy-vs-sqlite.md)
  - [libSQL embedded-local](research/libsql-embedded-local.md)
