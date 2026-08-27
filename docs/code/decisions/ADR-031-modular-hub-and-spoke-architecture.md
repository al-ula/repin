# ADR-031: Modular Hub-and-Spoke Architecture

```text
Status: accepted architecture and workspace topology decision
Date: 2026-08-27
Decision type: workspace crate decomposition, modular hub-and-spoke topology, pluggable language packs
Builds on: ADR-015, ADR-023, ADR-024, ADR-026, ADR-028
Supersedes: ADR-029, ADR-030 (decouples monolithic repin-core into hub-and-spoke capability crates)
Backs: docs/architecture.md, docs/api.md, docs/host-integration.md,
       docs/extraction.md, docs/conformance.md, docs/introduction.md
```

## 1. Context

ADR-030 consolidated capability crates into a single monolithic `repin-core` crate. While this simplified workspace metadata, bundling domain models, SQLite storage (`rusqlite`), filesystem/git bindings (`cap-std`), tree-sitter grammars (`tree-sitter-rust`, `tree-sitter-typescript`, `tree-sitter-md`), indexing, retrieval, and AI model providers into a single crate created heavy coupling.

Specific challenges with the monolithic approach:
1. **Heavyweight downstream embedding**: Any embedder or host application requiring only domain types, port contracts, or pure context packing was forced to pull in C bindings for SQLite and Tree-sitter.
2. **Language pack isolation**: Adding a new language extractor required compiling all other grammars, and language adapters could not be conditionally compiled or feature-gated cleanly.
3. **Storage engine coupling**: The indexing and retrieval algorithms depended directly on concrete storage types rather than port abstractions.

To resolve these challenges while maintaining clean isolation, Repin adopts a **modular hub-and-spoke architecture** inspired by the `hxui` reference model:
- `repin-core` is the zero-heavy-dependency contract hub.
- Capability adapters and language packs are isolated leaf crates (spokes) that depend strictly on `repin-core` and never on sibling functional crates.
- `repin-runtime` serves as the composition root, wiring spokes into the cohesive `Runtime` / `Engine` facade.
- `repin` delivers the product CLI and daemon.

## 2. Decision

The workspace is organized into a hub-and-spoke topology:

```text
                               ┌─────────────────┐
                               │   repin-core    │  (Hub: pure domain, traits, protocol)
                               └────────┬────────┘
             ┌───────────────┬──────────┼──────────┬───────────────┐
             │               │          │          │               │
     ┌───────▼───────┐ ┌─────▼─────┐ ┌──▼───┐ ┌────▼─────┐ ┌──────▼────────┐
     │repin-store-   │ │repin-packs│ │repin-│ │repin-    │ │repin-direct-  │
     │sqlite         │ │           │ │fs    │ │indexing  │ │search         │
     └───────┬───────┘ └─────┬─────┘ └──┬───┘ └────┬─────┘ └──────┬────────┘
             │               │          │          │              │
             └───────────────┼──────────┼──────────┼──────────────┘
                             │          │          │
             ┌───────────────┼──────────┼──────────┼──────────────┐
             │               │          │          │              │
     ┌───────▼───────┐ ┌─────▼─────┐ ┌──▼───┐      │              │
     │repin-retrieval│ │repin-     │ │repin-│      │              │
     │               │ │context    │ │intel.│      │              │
     └───────┬───────┘ └─────┬─────┘ └──┬───┘      │              │
             │               │          │          │              │
             └───────────────┼──────────┼──────────┘              │
                             │          │                         │
                       ┌─────▼──────────▼─────┐                   │
                       │    repin-runtime     │  (Composition Root)
                       └──────────┬───────────┘
                                  │
                           ┌──────▼──────┐
                           │    repin    │  (CLI & Daemon Binary)
                           └─────────────┘
```

### 2.1 Workspace Member Crate Roles & Boundaries

| Crate | Role | Allowed Dependencies | Prohibited Dependencies |
|---|---|---|---|
| `repin-core` | Zero-heavy-dependency contract hub: domain types, port traits, protocol envelopes, line indexing, versions, config, fact extractor utils | `serde`, `serde_json`, `toml`, `thiserror`, `tracing`, `blake3`, `hex` | `rusqlite`, `tree-sitter*`, `cap-std`, `ureq`, sibling crates |
| `repin-fs` | Filesystem capability, path containment, safety exclusions, Git VCS | `repin-core`, `cap-std`, `ignore`, `globset`, `tempfile` | Sibling spoke crates |
| `repin-store-sqlite` | SQLite/FTS5 storage adapter implementing `repin_core::ports::store::Store` | `repin-core`, `rusqlite` | Sibling spoke crates |
| `repin-direct-search` | Bounded working-tree regex/scanner search | `repin-core`, `regex`, `ignore` | Sibling spoke crates |
| `repin-packs` | Pluggable language packs implementing `repin_core::ports::pack::LanguagePack` | `repin-core`, `tree-sitter`, `tree-sitter-rust`, `tree-sitter-typescript`, `tree-sitter-md`, `pulldown-cmark` | Sibling spoke crates |
| `repin-indexing` | Indexing coordinator, invalidation, blast radius | `repin-core`, `rayon`, `crossbeam-channel` | Sibling spoke crates (e.g. no `rusqlite`, no `tree-sitter`) |
| `repin-retrieval` | Hybrid lexical/vector search, graph traversal, degree centrality, ranking | `repin-core` | Sibling spoke crates |
| `repin-context` | Evidence validation, token-budget packing, snippet formatting | `repin-core` | Sibling spoke crates |
| `repin-intelligence` | Remote API, embedded, and agent model providers | `repin-core`, optional `ureq` | Sibling spoke crates |
| `repin-runtime` | Composition root assembling spokes into `Runtime` / `Engine` facade | `repin-core`, `repin-fs`, `repin-store-sqlite`, `repin-direct-search`, `repin-packs`, `repin-indexing`, `repin-retrieval`, `repin-context`, `repin-intelligence` | `repin` |
| `repin` | Product CLI and daemon binary | `repin-core`, `repin-runtime`, `clap`, `tracing`, `tracing-subscriber`, `fs4`, `notify`, `tempfile` | None |

### 2.2 Strict Decoupling Invariants

1. **Spoke Isolation**: Functional spoke crates (`repin-fs`, `repin-store-sqlite`, `repin-direct-search`, `repin-packs`, `repin-indexing`, `repin-retrieval`, `repin-context`, `repin-intelligence`) MUST depend strictly on `repin-core` and MUST NOT depend on one another.
2. **Trait-Driven Indexing & Retrieval**: `repin-indexing` and `repin-retrieval` operate solely against `repin_core::ports::Store`, `repin_core::ports::ReadView`, `repin_core::ports::LanguagePack`, and `repin_core::ports::SourceFs`. They have zero compile-time knowledge of SQLite, FTS5, or Tree-sitter.
3. **Pluggable Language Packs**: `repin-packs` feature-gates language extractors (`rust`, `typescript`, `prose`). Downstream consumers can write custom language extractors in their own crates by implementing `repin_core::ports::pack::LanguagePack` without modifying `repin-packs` or `repin-core`.
4. **Single Composition Root**: `repin-runtime` is the sole crate that instantiates concrete store adapters, registers default language packs, and wires indexing and retrieval into a unified facade.

### 2.3 Compatibility Authorities

Compatibility authorities defined in ADR-024 remain fully invariant:

| Boundary | Authority |
| --- | --- |
| Package/API | Individual crate `CARGO_PKG_VERSION` |
| IPC | `repin-core` `protocol` module (`PROTOCOL_MIN` / `PROTOCOL_MAX`) |
| SQLite store | `repin-store-sqlite` / `repin-core` (`STORE_FORMAT_ID`, schema version) |
| Semantic facts | `repin-core` registries, `repin-packs` extractors |
| Build provenance | `repin` binary identity (`v<package>-<commit>`) |

## 3. Consequences

- **Lightweight Embedding**: Minimal embedded applications depend only on `repin-core` without pulling in SQLite, C toolchains, or Tree-sitter.
- **Fast Parallel Compilation**: Decoupled crates compile concurrently with clean dependency DAGs.
- **Extensibility**: Third-party language packs or alternate storage backends (e.g., in-memory or PostgreSQL) can be developed externally by implementing core port traits.
