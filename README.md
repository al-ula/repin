# Repin

**Rep**ository **in**telligence engine. A standalone, deterministic knowledge-graph engine for repositories.

## What it is

Repin builds and maintains a queryable graph of a repository — its files, symbols, documents, configuration, and the relationships between them — and keeps that graph current as files change. It also answers directly from the working tree, so it is useful before any index exists.

Repin is **deterministic** (no model, embedding, or network call required to build or query), **incremental** (a persistent graph updated in place is the normal mode; full indexing is the exception), and **standalone** (every client — CLI, agent harness, MCP server, editor plugin — is a thin adapter over one public API).

For the full scope, agnosticism, the workspace crate layout, and reading conventions, see the [Code and Architecture documentation](docs/code/introduction.md).

## Quick Start

### Build & Test

```bash
cargo build --release
cargo test
```

### Initialize, Index & Search

```bash
repin init                       # create .repin metadata in the repository root
repin index                      # deterministically index the working tree
repin search "my_function"       # direct working-tree text scan
repin search -g "EngineOptions"  # symbol graph declarations
repin context "how does daemon IPC work?" --budget 32768
```

Run `repin --help` for the full command reference.

## Documentation

Documentation is split by audience:

- [Usage Guide](docs/usage/index.md) — quick start, CLI reference, configuration, integrations, and troubleshooting.
- [Code and Architecture](docs/code/index.md) — architecture, contracts, data models, decisions, specifications, and verification.

The [`docs/SUMMARY.md`](docs/SUMMARY.md) is the mdBook navigation for both sections.
