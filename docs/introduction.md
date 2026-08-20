# Introduction

Repin is a standalone, deterministic knowledge-graph engine for repositories. It builds and maintains a queryable graph of a repository—its files, symbols, documents, configuration, and the relationships between them—and keeps that graph current as files change. It also answers directly from the working tree, so it is useful before any index exists.

## Status

**Implementation Complete & Authoritative Architecture Specification.** Research has concluded, all 16 Architectural Decision Records (ADRs) are finalized and accepted, and the complete Rust workspace implementation (10 crates) is in place. This book is the normative design, contract blueprint, and reference for Repin.

## What Repin is

Three properties define Repin:

1. **Deterministic.** The graph is constructed by parsers and resolvers. The same input always yields the same graph. No model, no embedding, and no network call is required to build or query it.
2. **Incremental.** A persistent graph updated in place is the normal mode of operation. Full indexing is the exception, not the routine.
3. **Standalone.** The engine owns construction, storage, querying, change tracking, result normalization, budgeting, and safety. Every client—CLI, agent harness, MCP server, editor plugin—is a thin adapter over one public API.

Two capabilities are exposed as one coherent product: direct retrieval from the working tree, and graph intelligence over derived structure. Direct retrieval works with no index at all; graph capabilities become available as the index exists. Callers always learn which subsystem answered, how fresh it is, and what evidence supports it.

Optional intelligence (embeddings, reranking, semantic enrichment) is a layer *above* the deterministic engine. It can be absent entirely, and nothing degrades except recall on fuzzy queries.

## What Repin is not

- Not a plugin for any host application, and not aware that any particular host exists.
- Not a language server, and not a substitute for one.
- Not a compiler or type checker. It records structure and references, not full semantics.
- Not an AI product. Intelligence is optional and additive.
- Not a hosted or remote service. It uses an on-demand local daemon for shared user-level runtime state; remote operation is a later deployment choice, not an assumption.

## Agnosticism

Two independent axes, both required:

**Implementation-language agnostic.** This book specifies architecture, contracts, and invariants—not a codebase. Interfaces are written in a neutral notation. Any host language capable of driving a parser and a transactional store can implement this design. Where a constraint originates in a real measurement, the measurement is cited but the constraint is stated in portable terms.

**Source-language agnostic.** No indexed language is privileged. Language support arrives as *language packs* behind one uniform contract (see [Extraction](extraction.md)). Source code is not privileged over prose either: documents, manifests, schemas, and configuration are first-class graph content.

**Driver and provider agnostic.** Storage, search indexes, file watching, and optional model capabilities are all *ports*. The core depends on the port, never on a product. No SQL, no vendor SDK, and no filesystem-notification API appears in core logic.

## How to read this book

The specification is organized into seven logical parts:

- **Part I: Architecture Foundations** ([Architecture](architecture.md), [Safety & Security Boundary](safety.md), [Results & Evidence Model](results.md)) defines the system layers, trust boundary, and output contract.
- **Part II: Core Domain & Data Model** ([Graph Model](graph-model.md), [Extraction](extraction.md), [Incremental Updates](incremental.md), [Storage](storage.md)) specifies what the engine stores, how facts are extracted and resolved, and how transactions and revisions guarantee convergence.
- **Part III: Query & Integration Surfaces** ([Retrieval](retrieval.md), [Public API](api.md), [Runtime & IPC](runtime.md), [Host Integration](host-integration.md), [Optional Intelligence](intelligence.md)) covers search channels, client contracts, daemon rendezvous, and host seams.
- **Part IV: Quality, Conformance & Implementation** ([Conformance](conformance.md), [Technology Selections & Implementation Profile](technology-candidates.md), [Roadmap](roadmap.md)) defines mechanical invariants, the accepted Rust/SQLite profile, and milestone delivery criteria.
- **Part V: Architectural Decision Records** ([Decisions](decisions/index.md)) contains the 16 accepted ADRs documenting the design rationale and constraints.
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
