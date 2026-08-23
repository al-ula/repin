# ADR-029: Consolidated crate topology

```text
Status: accepted architecture and library API decision
Date: 2026-08-22
Decision type: workspace crate responsibilities, public library surface, and product packaging
Builds on: ADR-015, ADR-023, ADR-024, ADR-028
Supersedes: ADR-023 crate topology (capability crates remain modules; the
            multi-crate extraction is withdrawn)
Backs: docs/architecture.md, docs/api.md, docs/host-integration.md,
       docs/conformance.md, docs/introduction.md
```

## 1. Context

ADR-023 extracted reusable capabilities into many workspace crates so an
embedded RAG host could depend on indexing, retrieval, and context without
the daemon or CLI. That extraction preserved semantics, but the workspace
grew to 16 packages. Most of those packages have a single consumer (the
default composition), small public surfaces, and a dense re-export facade
(`repin-runtime` / `repin-engine`). The extra crate graph costs more in
build metadata, import churn, and documentation than it returns in
independent reuse.

The product still needs a hard boundary between:

- a public library that any host can embed without Repin path policy, CLI,
  or daemon code;
- Repin-specific layout and host defaults shared by the product frontends;
- the CLI adapter;
- the daemon adapter;
- the installable executable.

Capability *boundaries* remain real (ports, algorithms, adapters,
composition). They do not each need a Cargo package.

## 2. Decision

The workspace has five crates:

```text
repin-core                 public library (domain, ports, protocol,
                           adapters, algorithms, default composition)
        ▲
        │  no product types
        │
repin-product              Repin path layout and host-default selection
        ▲
        ├──────────────────────────────┐
repin-cli                  CLI adapter │
        ▲                              │
        │                              │
repin-daemon               daemon      │
        ▲                              │
        └──────────────┬───────────────┘
                       │
repin                  thin executable (`[[bin]] name = "repin"`)
```

| Crate | Role | May depend on |
| --- | --- | --- |
| `repin-core` | Public, product-agnostic library | third-party crates only |
| `repin-product` | Product path names and host bases | `std` only (ADR-028) |
| `repin-cli` | CLI parsing, discovery, IPC client, command handlers | `repin-core`, `repin-product`, `repin-daemon` |
| `repin-daemon` | Per-user daemon, leases, project contexts | `repin-core`, `repin-product` |
| `repin` | Thin `main` that invokes the CLI | `repin-cli` |

`cargo install repin` installs the executable. Embedded hosts depend on
`repin-core`. They MUST NOT need `repin-cli`, `repin-daemon`, `repin-product`,
or the `repin` package.

### 2.1 `repin-core` ownership

`repin-core` owns every reusable library concern previously split across
`repin-core`, `repin-protocol`, `repin-fs`, `repin-store-sqlite`,
`repin-direct-search`, `repin-packs`, `repin-context`, `repin-retrieval`,
`repin-indexing`, `repin-intelligence`, `repin-runtime`, `repin-engine`, and
`repin-conformance`.

Internal modules preserve the former capability boundaries:

```text
repin-core
  config, hash, line_index, model, ports, versions
  protocol          result envelopes, error taxonomy, IPC values
  fs                CapabilityFs, exclusions, Git adapter
  store             SQLite/FTS5 Store adapter and identity constants
  direct_search     bounded working-tree scan
  packs             built-in LanguagePack implementations
  context           evidence validation and budget packing
  retrieval         graph, lexical, vector, ranking, traversal
  indexing          extraction and transactional update orchestration
  intelligence      optional embedded, agent, and remote providers
  runtime           default composition (`Runtime` / `Engine`)
  conformance       port suites and replay harness
```

Layer rules from [Architecture](../architecture.md) still apply inside the
crate: algorithms depend on port contracts, not on a product, protocol
client, or vendor SDK appearing above L0. Modules that implement a port
(`fs`, `store`, `packs`, `intelligence`) are adapters. Modules that consume
ports (`context`, `retrieval`, `indexing`, `direct_search`) MUST NOT select
a concrete store, filesystem, pack, or provider.

`runtime` is the sole default composition root. It constructs `CapabilityFs`,
`SqliteStore`, `GitVcs`, the built-in packs, and configured intelligence
providers, then owns high-level operation orchestration and result
normalization. `Engine` / `EngineOptions` remain public aliases of
`Runtime` / `RuntimeOptions`. There is no separate facade crate.

An embedded consumer depends on `repin-core` and may:

- use `Runtime` / `Engine` for Repin's default adapters; or
- call `context`, `retrieval`, `indexing`, and `direct_search` over
  caller-selected `ports` implementations, supplying its own `SourceFs`,
  `Store`, `LanguagePack`, `EmbeddingModel`, or `Reranker`.

The library returns the same normalized result envelopes, provenance,
freshness, coverage, warnings, redaction, cancellation, and truncation
metadata as the service path. Caller-owned inference remains intentional.

`repin-core` MUST NOT import `repin-product`, `repin-cli`, `repin-daemon`,
or `repin`. It accepts ordinary `Path` / `PathBuf` values, explicit roots,
and resolved `RepinConfig`. It performs no product path construction, no
`HOME` / XDG layout policy, and no CLI/daemon lifecycle.

### 2.2 Product crates

`repin-product` is unchanged in role from ADR-028: typed project, runtime,
and user layouts, plus host-default selection. No workspace-crate
dependencies. No filesystem I/O in layout constructors.

`repin-cli` owns clap parsing, project discovery, configuration file
loading, the IPC client, and command handlers. It exposes `run()` as the
process entry used by the executable. It does not define the `repin`
binary.

`repin-daemon` owns the Unix-socket server, singleton lease, context
registry, and daemon-mediated project state lifecycle.

`repin` contains `main`, the binary identity build script
(`REPIN_DISPLAY_VERSION`), and CLI contract tests that execute the binary.
It is a thin wrapper: parse nothing, compose nothing, call `repin_cli::run()`.

### 2.3 Features and compatibility authorities

Intelligence provider families remain independently feature-gated on
`repin-core` (`agent`, `embedded`, `remote`) where their dependencies
justify it. The default feature set stays offline-capable: no network
access, model download, or heavyweight embedded asset is enabled
implicitly by a deterministic operation.

Compatibility authorities from ADR-024 move with their owners, not with
the old package names:

| Boundary | Authority |
| --- | --- |
| Package/API | each Cargo package's `CARGO_PKG_VERSION` (diagnostic) |
| IPC | `repin-core` `protocol` module (`PROTOCOL_MIN` / `PROTOCOL_MAX`) |
| SQLite store | `repin-core` `store` module (`STORE_FORMAT_ID`, application ID, schema version) |
| Semantic facts | `repin-core` registries, packs, extractors |
| Build provenance | `repin` binary identity (`v<package>-<commit>`) |

No store-schema or IPC version bump is required for this packaging change.
Serialized envelopes, daemon protocol, and CLI flags are unchanged.

## 3. Withdrawn packages

These workspace members are removed. Their code lives as modules of
`repin-core` (or, for the former CLI binary, of `repin`):

```text
repin-protocol
repin-fs
repin-store-sqlite
repin-direct-search
repin-packs
repin-context
repin-retrieval
repin-indexing
repin-intelligence
repin-runtime
repin-engine
repin-conformance
```

There is no compatibility re-export crate. In-tree Rust code uses
`repin_core::…` paths. Out-of-tree embedders that depended on the withdrawn
package names must depend on `repin-core` (the public API was unpublished
for several of those crates).

## 4. Dependency rules

1. Dependencies point toward `repin-core` and `repin-product` only as shown
   in §2. `repin-core` has no workspace dependents inverted: nothing in
   `repin-core` may depend on a product crate.
2. `repin-daemon` MUST NOT depend on `repin-cli`. `repin-cli` MAY depend on
   `repin-daemon` to start or inspect the in-process server.
3. `repin` MUST NOT depend on `repin-core` or `repin-product` directly; it
   reaches them only through `repin-cli`.
4. Cargo metadata MUST show exactly the five workspace members above.
5. No in-process JSON or other serialization boundary is introduced between
   former crate modules.

## 5. Consequences

- Embedders have one public crate and one default composition type.
- The product keeps CLI and daemon as separate libraries with a shared
  layout crate and a thin installable binary.
- Capability algorithms remain independently callable through modules and
  port traits; they are no longer separately versioned packages.
- Documentation, CI package-name checks, and `cargo test -p` invocations
  that named withdrawn crates must target `repin-core`, `repin-cli`,
  `repin-daemon`, `repin-product`, or `repin`.

## 6. Acceptance

- `cargo metadata --no-deps --format-version 1` lists only the five
  members in §2.
- `repin-core` has no dependency on `repin-product`, `repin-cli`,
  `repin-daemon`, or `repin`.
- `repin-product` has no workspace-crate dependencies.
- `cargo test --workspace` passes, including former conformance, extraction,
  embedded-RAG, CLI, and daemon suites.
- `mdbook build docs/code` and `mdbook build docs/usage` pass.
- CLI identity, IPC range, and store schema constants are unchanged.
