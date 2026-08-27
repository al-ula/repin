# Repin

**Rep**ository **in**telligence engine. A standalone knowledge-graph engine for repositories.

## What it is

Repin builds and maintains a queryable graph of a repository — its files, symbols, documents, configuration, and the relationships between them — and keeps that graph current as files change. It also answers directly from the working tree, so it is useful before any index exists.

Repin is **incremental** (a persistent graph updated in place is the normal mode; full indexing is the exception), **offline-first** (no external model, embedding, or network call required to build or query), and **standalone** (every client — CLI, agent harness, MCP server, editor plugin — is a thin adapter over one public API).

For the full scope, agnosticism, the workspace crate layout, and reading conventions, see the [architecture specification](docs/code/introduction.md).

## Install

### CLI

```bash
curl -fsSL https://raw.githubusercontent.com/al-ula/repin/main/setup.sh | bash
```

### Agent Skill

Install the Repin skill for AI coding agents (Claude Code, Cursor, Codex, Pi, OpenCode, etc.):

```bash
npx skills add al-ula/repin -g -y
```

## Quick Start

### Build & Test

```bash
just release
just test
```

*(Or with direct Cargo: `cargo build --release && cargo test`)*

Set `CARGO_BUILD_TARGET` when producing a release for a specific target.

The `just` build recipes embed the current Git commit in the binary identity
(`v<package>-<commit>`, showing the first 12 characters). Direct Cargo builds
remain supported and use `v<package>-unknown`.

### Initialize, Index & Search

```bash
repin init                       # create .repin metadata in the repository root
repin index                      # index the working tree
repin search "my_function"       # direct working-tree text scan
repin search -g "EngineOptions"  # symbol graph declarations
repin context "how does daemon IPC work?" --budget 32768
```

Run `repin --help` for the full command reference.

## Supported Languages

Repin includes Tree-sitter powered AST symbol and relationship extraction packs:

| Language / Format | Extensions | Extracted Facts & Capabilities |
| --- | --- | --- |
| **Rust** | `.rs` | Structs, enums, traits, functions, methods, modules, type aliases, docs, imports, calls |
| **TypeScript / JavaScript** | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | Classes, interfaces, types, enums, functions, methods, JSDoc, imports/exports, calls |
| **Python** | `.py`, `.pyi`, `.pyw` | Classes, functions, methods, variables, docstrings, module imports |
| **Go** | `.go` | Packages, structs, interfaces, types, functions, methods, constants, variables, docs, imports |
| **C** | `.c`, `.h` | Structs, enums, types, functions, constants, variables, fields, docs, `#include` imports, calls |
| **C++** | `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` | Namespaces, classes, structs, enums, types, functions, methods, constructors, fields, docs, inheritance, imports, calls |
| **Java** | `.java` | Packages, classes, interfaces, enums, records, constructors, methods, constants, fields, docs, inheritance, imports, calls |
| **C#** | `.cs` | Namespaces, classes, interfaces, structs, enums, delegates, constructors, methods, properties, constants, fields, XML docs, inheritance, imports, calls |
| **Markdown / Prose** | `.md`, `.markdown` | Document sections, heading hierarchies, structural outlines |
| **Universal Text** | *All files* | Direct regular expression and full-text search across the entire working tree |
## Documentation

Two independent mdBooks:

- [User Guide](docs/usage/index.md) — quick start, CLI reference, configuration, agent workflows, and troubleshooting.
- [Architecture & Design Specification](docs/code/index.md) — architecture, contracts, data models, decisions, specifications, and verification.

Build them with `just docs` (or `mdbook build docs/code && mdbook build docs/usage`).

## License

Repin is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
