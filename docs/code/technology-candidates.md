# Technology Selections and Implementation Profile

The finalized implementation profile and adapter selections for Repin. This document details the accepted technologies, dependency pins, and validation requirements implementing the normative architecture.

```text
Status: accepted implementation profile and finalized architecture choices
Lifecycle stage: implementation (ready)
Decision authority: ADR-001 through ADR-016
```

## 1. Finalized technology selections

| Concern | Selected technology | Status | Role & ADR reference |
| --- | --- | --- | --- |
| Implementation language | Rust (2024 edition, stable) | accepted profile | Engine core, adapters, and standalone binary |
| Initial client | CLI with `clap` | accepted profile | Thin project-bound client and primary developer interface |
| Local IPC | Linux pathname Unix-domain sockets | accepted profile | Central per-user daemon rendezvous ([ADR-015](decisions/ADR-015-hybrid-per-user-daemon-runtime.md)) |
| Daemon singleton | OS-backed lease in private runtime directory | accepted profile | Exactly one on-demand daemon per OS user ([ADR-015](decisions/ADR-015-hybrid-per-user-daemon-runtime.md)) |
| Project registry | Canonical `.repin/graph.sqlite3` path | accepted profile | Active context registry key; no separate `ProjectId` ([ADR-015](decisions/ADR-015-hybrid-per-user-daemon-runtime.md)) |
| Authoritative store | SQLite 3.53.2 via `rusqlite` 0.40.1 (`bundled`) | accepted profile | Graph facts, metadata, revisions, recovery state ([ADR-009](decisions/ADR-009-sqlite-fts5-initial-profile.md)) |
| Lexical index | SQLite FTS5 (`detail=full`) | accepted profile | Transaction-coupled lexical search in same database ([ADR-009](decisions/ADR-009-sqlite-fts5-initial-profile.md)) |
| Alternative local engine | libSQL embedded-local | deferred research | Documented fallback for native vector evaluation at I5 |
| Vector index | Exact Rust scan over SQLite-backed embeddings | accepted I5 baseline | Optional semantic retrieval; implementation deferred to I5 ([ADR-012](decisions/ADR-012-exact-rust-vector-baseline.md)) |
| Parsing substrate | Language-native primary + Tree-sitter fallback | accepted architecture | Per-language extraction pipeline with text fallback ([ADR-013](decisions/ADR-013-native-parser-tree-sitter-fallback.md)) |
| Filesystem access | `cap-std` + `ignore` + `globset` | accepted profile | Root-confined capability access, ignore traversal, selection patterns ([ADR-003](decisions/ADR-003-capability-relative-filesystem.md)) |
| Content identity | BLAKE3 (algorithm-tagged) | accepted profile | Fast deduplication and cache hashes; not entity identity ([ADR-004](decisions/ADR-004-update-hash-protocol.md)) |
| Line index | Line starts + sparse Unicode checkpoints | accepted profile | Bounded byte-offset to public coordinate conversion ([ADR-014](decisions/ADR-014-sparse-checkpoint-line-index.md)) |
| Regex direct search | `regex` | accepted adapter | Bounded direct regex matching with exact spans ([ADR-010](decisions/ADR-010-regex-direct-search.md)) |
| VCS integration | Bounded Git subprocess | accepted adapter | Machine-readable changed-set and branch-state detection ([ADR-011](decisions/ADR-011-bounded-git-subprocess.md)) |
| Agent inspection & review | `inspectFile` + `AtPosition` + `reviewContext` | accepted profile | Structural outlines, position resolution, review composition ([ADR-016](decisions/ADR-016-agent-inspection-and-review-context.md)) |
| File watching | `notify` | deferred to I3 | Platform backends behind the `Watch` port ([ADR-007](decisions/ADR-007-optional-capability-sequencing.md)) |
| Writer exclusion | OS-backed advisory lock (`fs4` / platform adapter) | accepted profile | Atomic inter-process writer ownership with diagnostic metadata |
| Serialization | `serde` | accepted profile | Protocol and configuration encoding without exposing internal storage types |
| Diagnostics & tracing | `tracing` | accepted profile | Structured, redaction-aware instrumentation across all layers |
| Property testing | `proptest` | accepted profile | Generated convergence, identity stability, and coalescing sequences |
| Snapshot testing | `insta` + `assert_cmd` | accepted profile | Reviewable graph snapshots and black-box CLI validation |
| Fuzzing | `cargo-fuzz` + `libFuzzer` | accepted profile | Parsers, range conversions, path normalization, redaction, queries |
| Dependency & supply chain | `cargo-deny` + `cargo-audit` | accepted profile | License, source, duplicate, and vulnerability verification |
| Benchmarking | Criterion + `iai-callgrind` | accepted profile | Statistical end-to-end and deterministic instruction-count benchmarks |

All technology selections satisfy the port contracts defined in [Architecture](architecture.md). Core logic depends exclusively on abstract ports; no SQLite, FTS5, Git, or parser-specific types cross the port boundary into L1–L4.

## 2. Platform qualification scope

| Scope | Platform | Validation and support commitment |
|---|---|---|
| Initial PoC | Linux x86_64, glibc | Sole development and qualification target for the first implementation profile ([ADR-001](decisions/ADR-001-linux-poc-scope.md)). |

macOS, Windows, Linux musl/static builds, and additional architectures represent post-PoC platform expansion. They will be qualified during subsequent releases once the deterministic Linux implementation is complete.

### Rust toolchain and dependency baseline

Implementation uses the current stable Rust toolchain and Rust 2024 edition. The compiler, dependency versions, sources, features, and lockfile are recorded with each build:

- **Authoritative store & lexical:** `rusqlite = 0.40.1` (`default-features = false`, `features = ["bundled", "hooks"]`), bundling SQLite 3.53.2 from `libsqlite3-sys` 0.38.1.
- **Direct regex search:** `regex` with standard Unicode support and explicit compiled-size limits.
- **Direct VCS integration:** machine-readable Git subprocess with strict environment sanitization and bounded buffers.
- **Testing & verification:** `proptest`, `insta`, `assert_cmd`, `cargo-fuzz`, `cargo-deny`, `cargo-audit`.

## 3. Fixed architectural constraints

The implementation must strictly preserve these normative constraints:

- The working tree is authoritative for current file content.
- The `Store` port is authoritative for persisted graph facts and metadata.
- Lexical and vector indexes are derived, revisioned, replaceable, and rebuildable.
- Core logic depends on port contracts, never on concrete storage or parser libraries.
- Direct retrieval works without any index or graph store.
- Semantic retrieval is optional and cannot delay deterministic revisions.
- Exactly one authoritative writer owns a project graph; the global daemon holds the project lock, and failure to acquire it produces observer/direct-only mode with explicit `PROJECT_LEASE_UNAVAILABLE` status.
- Deleting `.repin` is a safe rebuild/reset only after its active project context has unloaded; active identity changes fail the context closed.

## 4. System composition

```text
CLI client (clap)
  └── project selector / initializer ── pathname Unix socket
                                        │
user daemon (same binary, detached on demand)
  ├── daemon lease + bounded connection acceptor ($XDG_RUNTIME_DIR/repin/daemon.sock)
  ├── canonical database-path context registry
  └── per-project context (.repin/)
       ├── Store port   ── SQLite 3.53.2 adapter (graph.sqlite3)
       ├── Lexical port ── SQLite FTS5 adapter (same transaction domain)
       ├── Line index   ── sparse-checkpoint ephemeral index
       ├── Vcs port     ── bounded Git subprocess
       ├── Regex port   ── regex crate direct matcher
       └── Vector port  ── exact Rust scan over SQLite rows (optional, I5)
```

The daemon acts as the composition root for normal operation. Its private runtime directory contains the central socket and singleton lease; each context owns its project's `.repin/writer.lock`, store, watcher, and derived indexes.

The in-process engine surface (`open(EngineOptions) -> Engine`) is retained for unit/integration tests, daemon internal composition, and standalone embedding.

### Workspace Crate Architecture

The Rust implementation is partitioned into two crates ([ADR-030](decisions/ADR-030-two-crate-workspace-topology.md)):

| Crate | Layer | Purpose |
| --- | --- | --- |
| `repin-core` | L0–L4 | Public library: domain models, port traits, result envelopes, IPC values, filesystem/store/pack adapters, retrieval, indexing, context, optional intelligence, default `Runtime`/`Engine`, conformance harness |
| `repin` | L4/L5 | Product library & binary: product layouts (`repin::product`), user daemon runtime (`repin::daemon`), CLI adapter (`repin::cli`), and executable (`cargo install repin`) |

## 5. Persistence and search architecture

### SQLite + FTS5 unified transaction domain

Under [ADR-009](decisions/ADR-009-sqlite-fts5-initial-profile.md), Repin uses SQLite in WAL mode (`synchronous=FULL`) with FTS5 in the same database:

- **Single transaction domain:** Graph mutations, revision increments, change history, and FTS5 index updates commit or roll back together atomically.
- **No cross-index lag:** Lexical updates cannot drift from graph revisions during deterministic execution.
- **Evidence re-verification:** Working tree bytes are re-read and verified against current content before lexical match regions are returned.
- **Direct regex independence:** Regex search executes directly over working tree bytes via the `regex` adapter ([ADR-010](decisions/ADR-010-regex-direct-search.md)) and does not use FTS5.

### Documented fallback profiles (non-primary)

The repository maintains detailed research records on alternative persistence candidates:

- **redb 4.1.0 + Tantivy 0.26.1:** Documented in [Research — redb + Tantivy versus SQLite](research/redb-tantivy-vs-sqlite.md). Preserved as a reference fallback if FTS5 encounters insurmountable lexical limitations.
- **libSQL embedded-local:** Documented in [Research — libSQL embedded-local](research/libsql-embedded-local.md). Preserved for re-evaluation during milestone I5 if native vector tables offer significant advantages over exact Rust scanning.

## 6. Parsing, line indexing, and inspection profiles

### Language-native parsers with Tree-sitter fallback

Under [ADR-013](decisions/ADR-013-native-parser-tree-sitter-fallback.md), each `LanguagePack` follows a three-tier extraction hierarchy:

1. **Primary native parser:** High-fidelity AST extraction (e.g. rust-analyzer family for Rust, Oxc/SWC for TypeScript/JavaScript).
2. **Recovery fallback parser:** Pinned Tree-sitter grammar when the primary parser encounters unrecoverable syntax errors.
3. **Text-only indexing:** Guaranteed discoverability via file-level indexing when structured parsing fails.

### Sparse-checkpoint line index

Under [ADR-014](decisions/ADR-014-sparse-checkpoint-line-index.md), byte offsets are converted to 1-based line and Unicode-scalar column coordinates using an ephemeral sparse-checkpoint line index:

- Stores logical line-start byte offsets.
- Records sparse scalar checkpoints with an initial 128-byte stride only on lines containing non-ASCII or invalid UTF-8 bytes.
- Decodes at most one stride on coordinate lookup.

### Agent inspection & review context

Under [ADR-016](decisions/ADR-016-agent-inspection-and-review-context.md), Repin exposes high-efficiency inspection endpoints:

- `inspectFile`: syntax outline, symbol declarations, and relations without reading full source bodies.
- `AtPosition`: resolves a source position to its exact or enclosing entity.
- `reviewContext`: packages changed files, reverse impact radius, and budgeted context into a single deterministic payload.
- Identifier sub-tokenization in FTS5 for compound identifier search (camelCase, snake_case, kebab-case).

## 7. Implementation validation tasks

Implementation proceeds directly against the following validation criteria:

1. **Store & Lexical Conformance:** Atomic commits, rollback safety, WAL checkpointing, foreign-key consistency, and FTS5 exact region verification under SQLite.
2. **Filesystem Capability Safety:** Fail-closed containment checks under component swap races using root-relative capability opens.
3. **Deterministic Convergence:** Property-tested update sequences verifying that incremental updates equal a clean re-index.
4. **Daemon Lifecycle & IPC:** Concurrent daemon candidate election, socket rendezvous, stale lease recovery, and clean idle context eviction.
5. **Direct Search Correctness:** Bounded regex execution, exact byte spans, and Git subprocess machine-readable parsing across all worktree states.
6. **Supply Chain Integrity:** Locked dependencies, license compliance via `cargo-deny`, and zero unreviewed vulnerabilities via `cargo-audit`.
