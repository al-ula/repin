# Repin

**Rep**ository **in**telligence engine. A standalone, deterministic knowledge-graph engine for repositories.

## Status

Planning. No implementation yet. This directory holds design documents only.

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
- Not a hosted or remote service. It uses an on-demand local daemon for shared
  user-level runtime state; remote operation is a later deployment choice, not
  an assumption.

## Agnosticism

Two independent axes, both required:

**Implementation-language agnostic.** These documents specify architecture, contracts, and invariants — not a codebase. Interfaces are written in a neutral notation. Any host language capable of driving a parser and a transactional store can implement this design. Where a constraint originates in a real measurement, the measurement is cited but the constraint is stated in portable terms.

**Source-language agnostic.** No indexed language is privileged. Language support arrives as *language packs* behind one uniform contract (see [Extraction](docs/extraction.md)). Source code is not privileged over prose either: documents, manifests, schemas, and configuration are first-class graph content.

**Driver and provider agnostic.** Storage, search indexes, file watching, and optional model capabilities are all *ports*. The core depends on the port, never on a product. No SQL, no vendor SDK, and no filesystem-notification API appears in core logic.

## Documents

| File | Contents |
|---|---|
| [Introduction](docs/introduction.md) | Product scope, status, agnosticism, and reading guidance |
| [Architecture](docs/architecture.md) | Layers, ports and adapters, dependency rules, deployment topologies |
| [Results and Evidence](docs/results.md) | Result envelope, evidence, entities, error taxonomy, output shaping |
| [Safety and Data Handling](docs/safety.md) | Path containment, exclusions, redaction, bounds, data egress |
| [Graph Model](docs/graph-model.md) | Nodes, edges, provenance, identity, kind and attribute registries, positions |
| [Extraction](docs/extraction.md) | Language packs, extractor contract, resolution, versioning |
| [Incremental Updates](docs/incremental.md) | Change model, transactions, revisions, invalidation, convergence |
| [Storage](docs/storage.md) | Storage, lexical, and vector ports; capability negotiation; migration |
| [Retrieval](docs/retrieval.md) | Retrieval channels, deterministic ranking, context construction |
| [Public API](docs/api.md) | Project-bound client surface, daemon-internal engine construction, errors, cancellation, stability policy |
| [Runtime and IPC](docs/runtime.md) | User daemon, local rendezvous, project discovery, bound connections, isolated contexts, and lifecycle |
| [Host Integration](docs/host-integration.md) | Adapter seam, capability negotiation, lifecycle, provider contract |
| [Optional Intelligence](docs/intelligence.md) | Optional capability ports and their asynchrony rules |
| [Conformance](docs/conformance.md) | Invariants, fixtures, conformance suites, benchmark method |
| [Roadmap](docs/roadmap.md) | Project lifecycle stages, implementation milestones, and exit criteria |
| [Technology Candidates](docs/technology-candidates.md) | Proposed Rust/CLI implementation direction and adapter candidates |
| [Planning Task Backlog](docs/tasks.md) | Byte-sized planning and experiment-preparation backlog |
| [Storage Adapter Experiments](docs/experiments/storage.md) | Planned storage-adapter and cross-index recovery experiments |
| [Rust Foundation Experiments](docs/experiments/rust-foundation.md) | Planned parser, filesystem, runtime, watch, and engineering-tool experiments |
| [Experiment Results](docs/experiments/results/index.md) | Result and recommendation ledger for every experiment family; pending runs are explicit |
| [Initial Fixture and Corpus Manifest](docs/experiments/fixtures.md) | Initial Rust/Markdown/TS/JS fixture families and workstation corpus bands |
| [Experiment Result Template](docs/experiments/template.md) | Reproducible experiment-result and evidence template |

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
