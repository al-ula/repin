# Summary

# Part I: Architecture Foundations

- [Introduction](introduction.md)
- [Architecture & Layers](architecture.md)
- [Safety & Security Boundary](safety.md)
- [Results & Evidence Model](results.md)

# Part II: Core Domain & Data Model

- [Graph Model & Invariants](graph-model.md)
- [Extraction & Language Packs](extraction.md)
- [Incremental Updates & Convergence](incremental.md)
- [Storage, Transactions & Persistence](storage.md)

# Part III: Query & Integration Surfaces

- [Retrieval, Ranking & Context](retrieval.md)
- [Public API Specification](api.md)
- [Runtime, IPC & Daemon Architecture](runtime.md)
- [Host Integration Seam](host-integration.md)
- [Optional Intelligence](intelligence.md)
- [Embedded RAG Proof](embedded-rag.md)

# Part IV: Quality, Conformance & Implementation

- [Conformance & Verification](conformance.md)
- [Reusable-library extraction baseline](benchmarks/library-extraction-baseline.md)
- [Technology Selections & Implementation Profile](technology-candidates.md)
- [Implementation Roadmap & Milestones](roadmap.md)

# Part V: Architectural Decision Records

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
  - [ADR-017 — Verbatim context & blast radius](decisions/ADR-017-verbatim-context-and-blast-radius.md)
  - [ADR-018 — Graph degree centrality in ranking](decisions/ADR-018-graph-degree-centrality-rank-fusion.md)
  - [ADR-019 — SQLite WAL checkpoint & compaction](decisions/ADR-019-sqlite-wal-checkpoint-and-compaction.md)
  - [ADR-020 — Schema string interning & JSON compression](decisions/ADR-020-schema-string-interning-and-compression.md)
  - [ADR-021 — Per-project configuration & merge protocol](decisions/ADR-021-per-project-configuration.md)
  - [ADR-022 — Multi-tier model provider architecture](decisions/ADR-022-multi-tier-model-providers.md)
  - [ADR-023 — Reusable capability crates and runtime facade](decisions/ADR-023-reusable-library-crates.md)
  - [ADR-024 — Compatibility versioning and conservative state replacement](decisions/ADR-024-compatibility-versioning.md)

# Part VI: Subsystem Specifications

- [Specification — Per-Project Configuration (`config.toml`)](specifications/project-configuration.md)
- [Specification — Sparse-checkpoint line index](specifications/sparse-line-index.md)
- [Specification — Language-native parsers with Tree-sitter fallback](specifications/native-parsers-tree-sitter-fallback.md)
- [Specification — Exact Rust vector search baseline](specifications/vector-search-rust-friendly.md)
- [Specification — Agent inspection and review context](specifications/agent-inspection-and-review-context.md)
- [Specification — Multi-Tier Model Provider Architecture](specifications/multi-tier-model-providers.md)

# Part VII: Concluded Research & Trade Studies

- [Research Record — redb + Tantivy versus SQLite + FTS5](research/redb-tantivy-vs-sqlite.md)
- [Research Record — libSQL embedded-local](research/libsql-embedded-local.md)
