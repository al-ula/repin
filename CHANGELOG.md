# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-08-26

### Added

- Event-driven background file watcher (`ProjectWatcher` via `notify`) in daemon runtime for active writer contexts, debouncing and coalescing filesystem mutations.
- Incremental single-file fact removal and update primitives in `repin-core` (`Engine::update_file`, `Engine::remove_file`, and `InvalidationCoordinator::apply_file_removal`).
- GitHub Pages automated deployment workflow for documentation portal and mdBooks.
- Version-matched changelog extraction for GitHub release notes.

### Changed

- Migrated `repin watch` command from synchronous VCS sync polling loops to status-driven revision observation backed by the daemon auto-watcher.
- Configured idle timeout override precedence for daemon context retention and shutdown.
- Consolidated workspace into a 2-crate topology (`repin-core` and `repin`) per ADR-030, merging CLI, daemon, and product layout modules into `repin`.

## [0.1.0] - 2026-08-23

### Added

- **Core Knowledge Graph Engine & Storage**:
  - SQLite and FTS5 transactional storage backend with WAL checkpointing, schema string interning, fact-owner surrogate keys, and JSON attribute compression (ADR-019, ADR-020).
  - Sparse-checkpoint line index (`LineIndex`, ADR-014) supporting ASCII, CRLF, and Unicode column offsets.
  - Deterministic node identity derivation, transaction isolation, and schema migration rollback handling (ADR-024).
- **Filesystem Containment & Exclusions**:
  - Capability-relative filesystem abstraction (`CapabilityFs`, ADR-003) enforcing strict working-tree path containment.
  - Config-aware and ignore-aware exclusion filter with immutable safety floors protecting credentials, VCS directories, and binary artifacts.
  - Bounded Git VCS integration for repository discovery, revision tracking, and dirty worktree change detection (ADR-011).
- **AST Language Extraction Packs**:
  - Tree-sitter extraction packs for Rust (`.rs`), TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`), and Markdown (`.md`, `.markdown`, `.txt`) (ADR-006, ADR-013).
  - Fact extraction for functions, structs, classes, traits, interfaces, impl blocks, modules, call graphs, imports/exports, and doc comments.
- **Search, Ranking & Graph Traversal**:
  - Direct indexless regular expression search across working trees (ADR-010).
  - FTS5 full-text lexical search and exact SIMD-friendly vector similarity baseline (ADR-012).
  - Graph degree centrality rank fusion (ADR-018) combining lexical match scores with graph topology.
  - Transitive blast-radius impact analysis (`repin impact`, ADR-017, ADR-025) via reverse BFS traversal.
  - Shortest dependency chain path traversal between symbols (`repin path`, ADR-025).
- **Context Packing & Agent Workflows**:
  - Deterministic token-budgeted context packing (`repin context`, ADR-016, ADR-017) assembling verbatim code slices with rank-ordered evidence.
  - Structural inspection commands (`repin inspect`, `repin graph`) for symbol metadata and dependency subgraphs.
  - Standalone Agent Skill specification (`skills/repin/SKILL.md`) for AI coding agents.
- **Daemon Runtime & IPC Protocol**:
  - Per-user Unix-domain socket daemon runtime (`repin-daemon`, ADR-015) with singleton file leases and idle auto-shutdown.
  - Daemon-mediated state lifecycle (ADR-026) for transactional project initialization, synchronization, and uninitialization.
  - Inode-based `DatabaseIdentity` guard failing closed on external database replacement.
  - Protocol negotiation across IPC client and daemon server with compatibility ranges (ADR-024).
- **Per-Project Configuration**:
  - TOML-based hierarchical configuration system (`repin.toml` / `.repin/config.toml`, ADR-021) with CLI flags and global credential isolation.
  - Subcommands: `repin config init`, `repin config show`, `repin config validate`.
- **Multi-Tier Model Provider Architecture**:
  - Optional model provider architecture (ADR-022) with port contracts for embedding, reranking, and text generation.
  - Tier 1 embedded ONNX local embeddings with Hugging Face Hub downloader.
  - Tier 2 agent-powered JSON-RPC reranking with deadline bounding.
  - Tier 3 remote API provider adapters (OpenAI, Ollama, Google Gemini).
  - Subcommands: `repin model download`, `repin model list`, `repin model remove`.
- **CLI & Distribution Tooling**:
  - Standalone CLI executable with subcommands: `init`, `index`, `sync`, `search`, `context`, `impact`, `path`, `inspect`, `graph`, `status`, `config`, `model`, `eval`, `daemon`, `install`, `update`, `check-update`, and `version`.
  - Self-installation and GitHub release update tooling (`repin install`, `repin update`, `setup.sh`) with target-specific tarball distribution.
  - Build provenance embedding Git commit identifiers into binary identity.
- **Documentation & Verification**:
  - Dual mdBook documentation suite: normative Architecture Specification (`docs/code`) and User Guide (`docs/usage`).
  - Documentation web landing portal (`docs/index.html`).
  - Conformance test harness, deterministic replay convergence tests, and cross-engine benchmark suite.

[Unreleased]: https://github.com/al-ula/repin/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/al-ula/repin/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/al-ula/repin/releases/tag/v0.1.0
