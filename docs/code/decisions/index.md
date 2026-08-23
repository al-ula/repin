# Decisions

This directory records the 29 accepted Architectural Decision Records (ADRs) establishing the architecture, contracts, and implementation profile for Repin.

Research has concluded for the accepted decisions. These ADRs bridge architectural requirements and production implementation.

## Decision Ledger

| ADR | Decision | Status | Basis |
| --- | --- | --- | --- |
| [ADR-001](ADR-001-linux-poc-scope.md) | Qualify the current PoC only on Linux x86_64/glibc; defer platform expansion | accepted scope decision | F1–F8 and Q-series Linux runs |
| [ADR-002](ADR-002-synchronous-core.md) | Use a synchronous core with explicit cancellation/deadline checks and bounded workers | accepted PoC default | F4 concurrency audit |
| [ADR-003](ADR-003-capability-relative-filesystem.md) | Require root-relative capability opens and fail closed on containment races | accepted contract decision | F-008, F-018 race benchmarks |
| [ADR-004](ADR-004-update-hash-protocol.md) | Use tagged content hashes and transactional stale-snapshot/update semantics | accepted contract decision | F-019 snapshot verification |
| [ADR-005](ADR-005-deterministic-search-contracts.md) | Keep direct regex/VCS behavior bounded, explicit, cancellable, and evidence-backed | accepted contract decision | F-014, F-015, F-020 |
| [ADR-006](ADR-006-extraction-and-ranges.md) | Preserve deterministic captures and byte/Unicode-scalar range semantics | accepted contract decision | F-017, F-004 range oracle |
| [ADR-007](ADR-007-optional-capability-sequencing.md) | Defer watching and vector search until their implementation milestones | accepted sequencing decision | F5 and S3 dispositions |
| [ADR-008](ADR-008-provisional-quality-and-content-policy.md) | Use fail-closed content checks and SPDX 2.3 as the PoC evidence format | accepted PoC policy | F-009, F7, Q-014 |
| [ADR-009](ADR-009-sqlite-fts5-initial-profile.md) | Use SQLite with FTS5 for the initial persistence profile | accepted implementation choice | secondary research & theoretical analysis |
| [ADR-010](ADR-010-regex-direct-search.md) | Use `regex` for initial direct regex search | accepted implementation choice | public documentation & contract comparison |
| [ADR-011](ADR-011-bounded-git-subprocess.md) | Use a bounded Git subprocess for initial VCS integration | accepted implementation choice | public documentation & contract comparison |
| [ADR-012](ADR-012-exact-rust-vector-baseline.md) | Use exact Rust vector search as the I5 baseline | accepted future implementation choice | proposal review & vector contract analysis |
| [ADR-013](ADR-013-native-parser-tree-sitter-fallback.md) | Prefer language-native parsers with Tree-sitter fallback | accepted extraction architecture | proposal review & contract analysis |
| [ADR-014](ADR-014-sparse-checkpoint-line-index.md) | Use a sparse-checkpoint line index | accepted implementation choice | proposal review & range evidence |
| [ADR-015](ADR-015-hybrid-per-user-daemon-runtime.md) | Use a per-user daemon with an in-process engine surface | accepted implementation architecture | runtime contract review & topology analysis |
| [ADR-016](ADR-016-agent-inspection-and-review-context.md) | Adopt agent inspection and change-review context profile | accepted contract and capability decision | proposal review & navigation analysis |
| [ADR-017](ADR-017-verbatim-context-and-blast-radius.md) | Verbatim source context packing and blast-radius summaries | accepted contract and capability decision | empirical benchmark & agent UX analysis |
| [ADR-018](ADR-018-graph-degree-centrality-rank-fusion.md) | Graph degree centrality in deterministic rank fusion | accepted contract and capability decision | empirical benchmark & ranking quality analysis |
| [ADR-019](ADR-019-sqlite-wal-checkpoint-and-compaction.md) | SQLite post-batch WAL checkpointing and storage compaction | accepted contract and capability decision | storage footprint audit & benchmark analysis |
| [ADR-020](ADR-020-schema-string-interning-and-compression.md) | Schema string interning and JSON attribute compression | accepted contract and capability decision | storage normalization, footprint & index efficiency |
| [ADR-021](ADR-021-per-project-configuration.md) | Per-project configuration file and precedence merge protocol | accepted contract and capability decision | system configuration, safety floor enforcement & ergonomics |
| [ADR-022](ADR-022-multi-tier-model-providers.md) | Multi-tier model provider architecture (Embedded, Agent, and APIs) | accepted contract and capability decision | offline privacy, agent pipelines & API interoperability |
| [ADR-023](ADR-023-reusable-library-crates.md) | Reusable capability crates and the runtime compatibility facade | accepted architecture and library API decision | embedded consumers, cycle-free composition & semantics-preserving extraction |
| [ADR-024](ADR-024-compatibility-versioning.md) | Compatibility versioning and conservative state replacement | accepted contract decision | independent package, IPC, store, semantic, and provenance boundaries |
| [ADR-025](ADR-025-graph-impact-and-path-traversal.md) | Graph impact analysis and dependency path traversal | accepted contract and capability decision | refactoring safety, blast radius analysis & dependency path tracing |
| [ADR-026](ADR-026-daemon-mediated-state-lifecycle.md) | Daemon-mediated state lifecycle and fail-closed database identity | accepted contract decision | writer-lease ownership, stale-inode fault analysis & registry identity guard |
| [ADR-027](ADR-027-cli-override-flags.md) | CLI flag overrides for per-invocation behavior tuning | accepted contract and capability decision | ADR-021 precedence layer, per-command override ergonomics |
| [ADR-028](ADR-028-centralized-path-layout.md) | Centralize Repin product paths and keep shared crates generic | accepted architecture and library boundary decision | concrete path audit, reusable-crate boundary |
| [ADR-029](ADR-029-consolidated-crate-topology.md) | Collapse reusable capabilities into `repin-core`; keep CLI, daemon, product, and binary as separate crates | accepted architecture and library API decision | crate-graph cost of ADR-023, product/library boundary |

## Implementation Validation Scope

With all architectural and product decisions accepted, remaining technical validation proceeds during implementation milestones:

- **Storage & Search Conformance (I0/I1):** Conformance test execution for Store and Lexical ports against SQLite 3.53.2 and FTS5, validating commit atomicity, WAL checkpointing, and exact region re-verification.
- **Runtime Fault Validation (I0/I1):** Fault injection testing for the per-user daemon under ADR-015, verifying concurrent elections, crash recovery, and socket cleanup.
- **Language Pack Extraction (I1):** Implementation-time selection of specific parser crates (e.g. rust-analyzer parser for Rust, Oxc/SWC for JS/TS) and Tree-sitter grammars under the accepted ADR-013 architecture.
- **Line Index Tuning (I1):** Private stride verification and benchmark validation under ADR-014.
- **Vector Search Baseline (I5):** Exact Rust scan implementation and precision/latency evaluation under ADR-012.

## Status Vocabulary

- **accepted contract decision** — normative behavior or safety scope is fixed across all implementations;
- **accepted implementation choice** — a concrete technology or adapter is selected from research and analysis;
- **accepted PoC default** — the implementation direction is chosen for the Linux PoC with an explicit revisit trigger;
- **accepted sequencing decision** — the milestone ordering and dependency gates are fixed.
