# Quick Start

The current implementation is qualified on Linux x86_64 with glibc. Build it with a current stable Rust toolchain and Git available on `PATH`.

## Build

From the repository root:

```bash
cargo build --release
cargo test
```

For a target-specific release build and distribution archive, set Cargo's
target explicitly:

```bash
CARGO_BUILD_TARGET=x86_64-unknown-linux-gnu just dist
```

The binary is written to `target/<target>/release/repin` when
`CARGO_BUILD_TARGET` is set.

The setup script selects the host target automatically. Set `REPIN_TARGET`
only when selecting a published compatible target archive explicitly.

Use `target/release/repin` directly or put it on `PATH`:

```bash
install -Dm755 target/release/repin "$HOME/.local/bin/repin"
```

## Index a repository

Run these commands from the repository you want to inspect:

```bash
repin init
repin status
```

`repin init` creates `.repin` metadata and indexes the repository unless `--no-index` is supplied. To separate setup from indexing:

```bash
repin init --no-index
repin index
```

Repin starts or connects to its per-user daemon as needed. The daemon owns shared project state; the project database lives at `.repin/graph.sqlite3`.

## Supported languages

Repin includes built-in language extractors for syntax, declarations, imports, doc comments, and type/call dependencies:

| Language | Extensions | Extracted constructs |
| --- | --- | --- |
| **Rust** | `.rs` | structs, enums, traits, functions, methods, modules, docs, import references |
| **TypeScript / JavaScript** | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | classes, interfaces, types, enums, functions, methods, JSDoc, module imports |
| **Python** | `.py`, `.pyi`, `.pyw` | classes, functions, methods, variables, docstrings, imports |
| **Go** | `.go` | packages, structs, interfaces, types, functions, methods, constants, variables, docs, imports |
| **C** | `.c`, `.h` | structs, enums, types, functions, constants, variables, fields, docs, `#include` imports, calls |
| **C++** | `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` | namespaces, classes, structs, enums, types, functions, methods, constructors, fields, docs, inheritance, imports, calls |
| **Java** | `.java` | packages, classes, interfaces, enums, records, constructors, methods, constants, fields, docs, inheritance, imports, calls |
| **C#** | `.cs` | namespaces, classes, interfaces, structs, enums, delegates, constructors, methods, properties, constants, fields, XML docs, inheritance, using imports, calls |
| **Markdown** | `.md`, `.markdown` | documents, sections, heading hierarchy |

Files in unsupported languages degrade gracefully to text-searchable file nodes rather than being skipped.

## Search and inspect

```bash
repin search "connection pool"
repin search -r 'fn [a-z_]+\('
repin search -g "DaemonClient"
repin inspect src/main.rs
repin at-position src/main.rs 42 10
```

The default search mode comes from configuration and is `hybrid` by default. Use `-r` for a direct working-tree regular-expression search, `-g` for graph symbol search, and `--hybrid` to select both channels explicitly.

For graph navigation and agent-ready context:

```bash
repin entity "DaemonClient"
repin neighbors "DaemonClient" --max-depth 2
repin impact "DaemonClient"
repin path "DaemonClient" "DaemonRegistry"
repin context "How does daemon IPC work?" --budget 32768
repin review-context --since 1 --budget 65536
```

## Keep the index current

```bash
repin sync
repin watch --interval 1000
```

Use `sync` after a batch of changes. Use `watch` while actively editing. Rebuild a specific derived view when needed:

```bash
repin rebuild graph
repin rebuild lexical
repin rebuild vector
repin rebuild all
```

## Stop using Repin

Stop or restart the daemon without removing project data:

```bash
repin daemon status
repin daemon stop
repin daemon restart
```

Remove the project metadata after confirming the prompt:

```bash
repin uninit
```

Use `repin uninit --force` for a non-interactive removal.
