# Repin

**Rep**ository **in**telligence engine. A standalone, deterministic knowledge-graph engine for repositories.

## Status

**Implementation Complete & Authoritative Architecture Specification.** Research has concluded and all 16 Architectural Decision Records (ADRs) are finalized and accepted. This repository contains the complete normative design, port contracts, data models, and a full Rust workspace implementation (10 crates) ready for production and agent harness integration.

## What it is

Repin builds and maintains a queryable graph of a repository — its files, symbols, documents, configuration, and the relationships between them — and keeps that graph current as files change. It also answers directly from the working tree, so it is useful before any index exists.

Three properties define it:

1. **Deterministic.** The graph is constructed by parsers and resolvers. The same input always yields the same graph. No model, no embedding, and no network call is required to build or query it.
2. **Incremental.** A persistent graph updated in place is the normal mode of operation. Full indexing is the exception, not the routine.
3. **Standalone.** The engine owns construction, storage, querying, change tracking, result normalization, budgeting, and safety. Every client — CLI, agent harness, MCP server, editor plugin — is a thin adapter over one public API.

Two capabilities are exposed as one coherent product: direct retrieval from the working tree, and graph intelligence over derived structure. Direct retrieval works with no index at all; graph capabilities become available as the index exists. Callers always learn which subsystem answered, how fresh it is, and what evidence supports it.

Optional intelligence (embeddings, reranking, semantic enrichment) is a layer *above* the deterministic engine. It can be absent entirely, and nothing degrades except recall on fuzzy queries.

## What it is not

- Not a plugin for any host application, and not aware that any particular host exists.
- Not a language server, and not a substitute for one.
- Not a compiler or type checker. It records structure and references, not full semantics.
- Not an AI product. Intelligence is optional and additive.
- Not a hosted or remote service. It uses an on-demand local daemon for shared user-level runtime state; remote operation is a later deployment choice, not an assumption.

## Agnosticism

Two independent axes, both required:

**Implementation-language agnostic.** These documents specify architecture, contracts, and invariants — not a codebase. Interfaces are written in a neutral notation. Any host language capable of driving a parser and a transactional store can implement this design. Where a constraint originates in a real measurement, the measurement is cited but the constraint is stated in portable terms.

**Source-language agnostic.** No indexed language is privileged. Language support arrives as *language packs* behind one uniform contract (see [Extraction](docs/extraction.md)). Source code is not privileged over prose either: documents, manifests, schemas, and configuration are first-class graph content.

**Driver and provider agnostic.** Storage, search indexes, file watching, and optional model capabilities are all *ports*. The core depends on the port, never on a product. No SQL, no vendor SDK, and no filesystem-notification API appears in core logic.

## Workspace Crates

The codebase is organized into 10 focused Rust crates in a single workspace:

| Crate | Purpose | Key Responsibilities |
|---|---|---|
| [`repin-core`](crates/repin-core) | Core domain & ports | Domain models (`Node`, `Edge`, `Identity`, `Position`), `LineIndex`, hash protocols, port traits |
| [`repin-protocol`](crates/repin-protocol) | Protocol & serialization | Result envelopes, status codes, IPC request/response framing |
| [`repin-fs`](crates/repin-fs) | Filesystem & VCS | `cap-std` root-confined access, default exclusions, Git subprocess adapter |
| [`repin-store-sqlite`](crates/repin-store-sqlite) | Storage & lexical engine | Bundled SQLite 3.53.2 in WAL mode, FTS5 lexical index, transactional read/write views |
| [`repin-direct-search`](crates/repin-direct-search) | Direct regex retrieval | Bounded working tree direct regex search engine (`regex` adapter) |
| [`repin-packs`](crates/repin-packs) | Language extractors | Language packs (Rust, TypeScript/JavaScript, Markdown) with Tree-sitter & AST fallbacks |
| [`repin-engine`](crates/repin-engine) | Engine composition | In-process engine, deterministic ranker, AST inspector, context builder, graph traversals, exact vector index, eval harness, agent reranker |
| [`repin-daemon`](crates/repin-daemon) | User daemon server | Background daemon runtime, Unix domain socket rendezvous, per-project writer lease coordination |
| [`repin-cli`](crates/repin-cli) | CLI frontend | Project discovery, daemon auto-connect, rich developer and agent commands (`repin`) |
| [`repin-conformance`](crates/repin-conformance) | Conformance & verification | Automated port conformance tests, replay convergence harness, property test fixtures |

## Quick Start & CLI Reference

### Building & Testing

```bash
# Build binary
cargo build --release

# Run all workspace tests and conformance suites
cargo test
```

### Initializing & Indexing a Repository

```bash
# Initialize .repin metadata in the repository root
repin init

# Index the working tree deterministically
repin index
```

### Search & Retrieval

```bash
# Search using direct working-tree text scan
repin search "my_function"

# Search using direct regular expression (working tree)
repin search -r "fn [a-z_]+\("

# Search symbol graph declarations
repin search -g "EngineOptions"

# Search using deterministic hybrid multi-channel fusion (FTS + Symbol graph)
repin search --hybrid "store database" --limit 20
```

### Code Inspection & Resolution

```bash
# Inspect structural AST outline and declared symbols of a file
repin inspect src/main.rs

# Resolve AST symbol definition at specific line & column coordinate
repin at-position src/main.rs 42 10
```

### Graph Traversal & Context Construction

```bash
# Look up detailed metadata for an entity by name or node ID
repin entity "DaemonClient"

# Display graph relationship neighbors (callers, callees, definitions)
repin neighbors "DaemonClient" --max-depth 2

# Construct budgeted context packed for LLM / AI agent consumption
repin context "How does daemon IPC connection work?" --budget 32768

# Construct change-review context focused on changed files and impact (ADR-016)
repin review-context --since 1 --budget 65536
```

### AI Agent Shell Callback Reranking

```bash
# Rerank retrieved candidate symbols using an agent shell callback (e.g. Antigravity agy or custom script)
repin rerank "sqlite transaction" --agent-cmd 'agy -p "$(cat)"'
```

### Incremental Updates & Watching

```bash
# Incrementally synchronize graph from VCS worktree changes
repin update

# Continuously watch repository worktree for changes
repin watch --interval 1000
```

### Daemon Management & Diagnostics

```bash
# Check daemon connection, graph revision, and index status
repin status

# Check daemon process and socket state
repin daemon status

# Stop / restart running background daemon
repin stop
repin restart

# Run Precision-at-N retrieval evaluation suite
repin eval
```

## Documentation Structure

The complete specification is organized into seven parts:

### Part I: Architecture Foundations

- [Introduction](docs/introduction.md) — Product scope, status, agnosticism, and reading guidance
- [Architecture & Layers](docs/architecture.md) — Six-layer model, ports and adapters, dependency rules, deployment topologies
- [Safety & Security Boundary](docs/safety.md) — Path containment, exclusions, redaction, bounds, and data egress
- [Results & Evidence Model](docs/results.md) — Result envelope, evidence, entities, error taxonomy, output shaping

### Part II: Core Domain & Data Model

- [Graph Model & Invariants](docs/graph-model.md) — Nodes, edges, provenance, identity, kind/attribute registries, positions
- [Extraction & Language Packs](docs/extraction.md) — Language packs, extractor contract, resolution, versioning
- [Incremental Updates & Convergence](docs/incremental.md) — Change model, transactions, revisions, invalidation, convergence
- [Storage, Transactions & Persistence](docs/storage.md) — Storage, lexical, and vector ports; capability negotiation; migration

### Part III: Query & Integration Surfaces

- [Retrieval, Ranking & Context](docs/retrieval.md) — Retrieval channels, deterministic ranking, context construction
- [Public API Specification](docs/api.md) — Project-bound client surface, daemon-internal engine construction, errors, cancellation
- [Runtime, IPC & Daemon Architecture](docs/runtime.md) — User daemon, local rendezvous, project discovery, bound connections, isolated contexts, lifecycle
- [Host Integration Seam](docs/host-integration.md) — Adapter seam, capability negotiation, lifecycle, provider contract
- [Optional Intelligence](docs/intelligence.md) — Optional capability ports and their asynchrony rules

### Part IV: Quality, Conformance & Implementation

- [Conformance & Verification](docs/conformance.md) — Invariants, fixtures, conformance suites, benchmark method
- [Technology Selections & Implementation Profile](docs/technology-candidates.md) — Finalized Rust/CLI implementation profile and adapter selections
- [Implementation Roadmap & Milestones](docs/roadmap.md) — Implementation milestones I0–I8 and exit criteria

### Part V: Architectural Decision Records

- [Decision Ledger](docs/decisions/index.md) — All 16 accepted ADRs (ADR-001 through ADR-016)

### Part VI: Subsystem Specifications

- [Specification — Sparse-checkpoint line index](docs/specifications/sparse-line-index.md)
- [Specification — Language-native parsers with Tree-sitter fallback](docs/specifications/native-parsers-tree-sitter-fallback.md)
- [Specification — Exact Rust vector search baseline](docs/specifications/vector-search-rust-friendly.md)
- [Specification — Agent inspection and review context](docs/specifications/agent-inspection-and-review-context.md)

### Part VII: Concluded Research & Trade Studies

- [Research Record — redb + Tantivy versus SQLite + FTS5](docs/research/redb-tantivy-vs-sqlite.md)
- [Research Record — libSQL embedded-local](docs/research/libsql-embedded-local.md)

Read [Architecture](docs/architecture.md) first. Every other document assumes its vocabulary. [Graph Model](docs/graph-model.md) and [Incremental Updates](docs/incremental.md) carry the load-bearing invariants; if you read only three documents, read those two after the architecture.

## Reading conventions

Interface definitions use a neutral notation, not any real language:

```text
PortName
  operation(param: Type, param: Type) -> ReturnType
  operation(param: Type) -> Result<Ok, Error>
```

Types like `Path`, `Bytes`, `Hash`, and `Revision` are abstract. Their concrete representation is an implementation choice, constrained only where a document says so explicitly.

`MUST`, `MUST NOT`, and `SHOULD` carry their ordinary specification force. Everything else is guidance.
