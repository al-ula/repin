# Introduction

Repin is a standalone, deterministic knowledge-graph engine for repositories. It builds and maintains a queryable graph of a repository—its files, symbols, documents, configuration, and the relationships between them—and keeps that graph current as files change. It also answers directly from the working tree, so it is useful before any index exists.

## Status

**Implementation Complete & Authoritative Architecture Specification.** Research has concluded, all 31 Architectural Decision Records (ADRs) are finalized and accepted, and the complete Rust workspace implementation (11 crates) is in place. This section is the normative design, contract blueprint, and implementation reference for Repin.

## What Repin is

Three properties define Repin:

1. **Deterministic.** The graph is constructed by parsers and resolvers. The same input always yields the same graph. No model, no embedding, and no network call is required to build or query it.
2. **Incremental.** A persistent graph updated in place is the normal mode of operation. Full indexing is the exception, not the routine.
3. **Standalone.** The engine owns construction, storage, querying, change tracking, result normalization, budgeting, and safety. Every client—CLI, agent harness, MCP server, editor plugin—is a thin adapter over one public API.

Two capabilities are exposed as one coherent product: direct retrieval from the working tree, and graph intelligence over derived structure. Direct retrieval works with no index at all; graph capabilities become available as the index exists. Callers always learn which subsystem answered, how fresh it is, and what evidence supports it.

Optional intelligence (embeddings, reranking, semantic enrichment) is a layer *above* the deterministic engine. It can be absent entirely, and nothing degrades except recall on fuzzy queries.

## Agnosticism

Two independent axes, both required:

**Implementation-language agnostic.** This book specifies architecture, contracts, and invariants—not a codebase. Interfaces are written in a neutral notation. Any host language capable of driving a parser and a transactional store can implement this design. Where a constraint originates in a real measurement, the measurement is cited but the constraint is stated in portable terms.

**Source-language agnostic.** No indexed language is privileged. Language support arrives as *language packs* behind one uniform contract (see [Extraction](extraction.md)). Source code is not privileged over prose either: documents, manifests, schemas, and configuration are first-class graph content.

**Driver and provider agnostic.** Storage, search indexes, file watching, and optional model capabilities are all *ports*. The core depends on the port, never on a product. No SQL, no vendor SDK, and no filesystem-notification API appears in core logic.

## Workspace Crates

The implementation is organized into a modular hub-and-spoke architecture across 11 Rust crates in a single workspace ([ADR-031](decisions/ADR-031-modular-hub-and-spoke-architecture.md)):

| Crate | Purpose | Key Responsibilities |
| --- | --- | --- |
| [`repin-core`](../../crates/repin-core) | Contract hub | Pure domain models, port traits, protocol envelopes, line indexing, versions, config, and extractor utilities with zero heavy dependencies |
| [`repin-fs`](../../crates/repin-fs) | Filesystem capability | Filesystem capability adapters, path containment, immutable safety exclusions, and Git VCS integration |
| [`repin-store-sqlite`](../../crates/repin-store-sqlite) | Storage adapter | SQLite and FTS5 storage adapter implementing `repin_core::ports::store::Store` |
| [`repin-direct-search`](../../crates/repin-direct-search) | Direct retrieval | Bounded working-tree regex and scanner search over source files |
| [`repin-packs`](../../crates/repin-packs) | Language packs | Pluggable language extractors (Rust, TS/JS, Python, Go, C, C++, Java, C#, Markdown/prose) implementing `LanguagePack` |
| [`repin-indexing`](../../crates/repin-indexing) | Indexing orchestration | Indexing coordinator, invalidation planning, and transactional update orchestration |
| [`repin-retrieval`](../../crates/repin-retrieval) | Retrieval algorithms | Hybrid lexical/vector search, graph traversal, degree centrality, and deterministic rank fusion |
| [`repin-context`](../../crates/repin-context) | Context construction | Evidence validation, deterministic token-budget packing, and snippet formatting |
| [`repin-intelligence`](../../crates/repin-intelligence) | Model providers | AI model provider adapters (embedded ONNX, agent CLI callback, remote REST APIs) |
| [`repin-runtime`](../../crates/repin-runtime) | Composition root | Assembles spokes into the `Runtime` facade (`Engine` compatibility alias) |
| [`repin`](../../crates/repin) | Product library & binary | Product path layouts (`repin::product`), user daemon server and leases (`repin::daemon`), CLI parsing and subcommands (`repin::cli`), and installable executable |

Capability spokes depend strictly on `repin-core` and never on sibling functional crates. An embedded host can depend on `repin-runtime` for full composition or select specific spokes with `repin-core`. The Repin product and executable live in `repin`.
## How to read this book

The specification is organized into seven logical parts:

- **Part I: Architecture Foundations** ([Architecture](architecture.md), [Safety & Security Boundary](safety.md), [Results & Evidence Model](results.md)) defines the system layers, trust boundary, and output contract.
- **Part II: Core Domain & Data Model** ([Graph Model](graph-model.md), [Extraction](extraction.md), [Incremental Updates](incremental.md), [Storage](storage.md)) specifies what the engine stores, how facts are extracted and resolved, and how transactions and revisions guarantee convergence.
- **Part III: Query & Integration Surfaces** ([Retrieval](retrieval.md), [Public API](api.md), [Runtime & IPC](runtime.md), [Host Integration](host-integration.md), [Optional Intelligence](intelligence.md)) covers search channels, client contracts, daemon rendezvous, and host seams.
- **Part IV: Quality, Conformance & Implementation** ([Conformance](conformance.md), [Technology Selections & Implementation Profile](technology-candidates.md), [Roadmap](roadmap.md)) defines mechanical invariants, the accepted Rust/SQLite profile, and milestone delivery criteria.
- **Part V: Architectural Decision Records** ([Decisions](decisions/index.md)) contains the 31 accepted ADRs documenting the design rationale and constraints.
- **Part VI: Subsystem Specifications** ([Line Index](specifications/sparse-line-index.md), [Native Parsers](specifications/native-parsers-tree-sitter-fallback.md), [Vector Baseline](specifications/vector-search-rust-friendly.md), [Agent Context](specifications/agent-inspection-and-review-context.md)) provides deep normative algorithmic specifications.
- **Part VII: Concluded Research & Trade Studies** ([redb vs SQLite](research/redb-tantivy-vs-sqlite.md), [libSQL](research/libsql-embedded-local.md)) documents research and candidate evaluations.

Start with [Architecture](architecture.md). It defines the vocabulary and the boundaries that the remaining documents assume. Then read [Results and Evidence](results.md), [Safety and Data Handling](safety.md), [Graph Model](graph-model.md), and [Extraction](extraction.md) to understand what the engine stores, returns, and refuses to do. [Incremental Updates](incremental.md) and [Storage](storage.md) specify how those facts remain current and durable.

## Reading conventions

Interface definitions use a neutral notation, not any real language:

```text
PortName
  operation(param: Type, param: Type) -> ReturnType
  operation(param: Type) -> Result<Ok, Error>
```

Types like `Path`, `Bytes`, `Hash`, and `Revision` are abstract. Their concrete representation is an implementation choice, constrained only where a document says so explicitly.

`MUST`, `MUST NOT`, and `SHOULD` carry their ordinary specification force. Everything else is guidance.
