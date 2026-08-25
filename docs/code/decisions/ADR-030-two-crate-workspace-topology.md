# ADR-030: Two-crate workspace topology

```text
Status: accepted architecture and library API decision
Date: 2026-08-22
Decision type: workspace crate consolidation, packaging, and binary integration
Builds on: ADR-015, ADR-024, ADR-026, ADR-028, ADR-029
Supersedes: ADR-029 workspace topology (merges repin-cli, repin-daemon, and repin-product into repin)
Backs: docs/architecture.md, docs/api.md, docs/host-integration.md,
       docs/conformance.md, docs/introduction.md
```

## 1. Context

ADR-029 consolidated reusable capability crates into `repin-core` while keeping `repin-product`, `repin-daemon`, `repin-cli`, and `repin` as separate workspace crates. In practice, `repin-product`, `repin-daemon`, and `repin-cli` are not independently published or reused outside the Repin product itself; they form the single cohesive Repin application and daemon runtime. Maintaining four product-tier crates adds Cargo metadata overhead, inter-crate dependency glue, and build churn without distinct external consumers.

The system requires two distinct packaging boundaries:

1. **`repin-core`**: The public, embeddable, product-agnostic library containing domain models, port traits, storage/filesystem/pack adapters, indexing, retrieval, context packing, and the default `Runtime`/`Engine` composition.
2. **`repin`**: The product crate and executable containing product path policy, daemon server and lease management, CLI commands, application dispatch, and the installable binary.

## 2. Decision

The workspace consists of exactly two crates:

```text
repin-core                 public library (domain, ports, protocol,
                           adapters, algorithms, default composition)
        ▲
        │  no product dependencies
        │
repin                      product library and executable
                             ├── repin::product  (path layouts and host bases)
                             ├── repin::daemon   (per-user daemon, leases, state lifecycle)
                             └── repin::cli      (CLI parsing, discovery, IPC client, commands)
```

| Crate | Role | Dependencies |
| --- | --- | --- |
| `repin-core` | Public, product-agnostic library | third-party crates only |
| `repin` | Product library and binary (`[[bin]] name = "repin"`) | `repin-core`, third-party crates |

`cargo install repin` installs the standalone binary. Embedded RAG applications and custom hosts depend solely on `repin-core`.

### 2.1 `repin-core` ownership

`repin-core` continues to own all product-agnostic engine capabilities:

- `config`, `hash`, `line_index`, `model`, `ports`, `versions`
- `protocol`: result envelopes, error taxonomy, IPC message types
- `fs`: `CapabilityFs`, exclusions, Git VCS adapter
- `store`: SQLite/FTS5 store adapter and schema invariants
- `direct_search`: bounded working-tree search
- `packs`: built-in language pack implementations
- `context`: evidence validation and token-budget packing
- `retrieval`: graph, lexical, vector, ranking, traversal
- `indexing`: transactional update coordination
- `intelligence`: optional model provider adapters
- `runtime`: default composition (`Runtime` / `Engine`)
- `conformance`: port suites and replay harness

`repin-core` MUST NOT depend on `repin` or any product layout/CLI/daemon module.

### 2.2 `repin` crate structure

The `repin` crate encapsulates product-level subsystems into clear internal modules:

```text
repin
  src/
    lib.rs             crate root and public re-exports
    main.rs            executable entry point (invokes repin::run())
    product.rs         product path layouts (ProjectLayout, RuntimeLayout, UserLayout)
    daemon/            daemon server, lease coordination, context registry, state lifecycle
      context_handle.rs
      lease.rs
      registry.rs
      server.rs
      state.rs
    cli/               CLI application, discovery, IPC client, and subcommands
      app.rs
      client.rs
      discovery.rs
      commands/
```

- `repin::product` provides deterministic, typed path layouts without side effects.
- `repin::daemon` manages the Unix-domain socket server, singleton lease election, context registry, and state lifecycle.
- `repin::cli` manages CLI parsing, project discovery, IPC communication with the daemon, and local command execution.
- `repin::run()` provides the main process dispatch.

### 2.3 Compatibility authorities

Compatibility authorities from ADR-024 remain invariant:

| Boundary | Authority |
| --- | --- |
| Package/API | `repin-core` and `repin` `CARGO_PKG_VERSION` (diagnostic) |
| IPC | `repin-core` `protocol` module (`PROTOCOL_MIN` / `PROTOCOL_MAX`) |
| SQLite store | `repin-core` `store` module (`STORE_FORMAT_ID`, application ID, schema version) |
| Semantic facts | `repin-core` registries, packs, extractors |
| Build provenance | `repin` binary identity (`v<package>-<commit>`) |

No store schema or IPC protocol changes are introduced.

## 3. Withdrawn packages

The following packages are merged into `repin`:

- `repin-product` -> `repin::product`
- `repin-daemon` -> `repin::daemon`
- `repin-cli` -> `repin::cli`

## 4. Dependency rules

1. `repin-core` MUST NOT depend on `repin`.
2. `repin` depends on `repin-core` and necessary third-party crates.
3. Cargo metadata MUST list exactly two workspace members: `repin-core` and `repin`.
4. No additional serialization boundaries or performance overhead are introduced.

## 5. Consequences

- Clean 2-crate workspace structure: one library crate for embedders, one application crate for the product binary and daemon.
- Simplified dependency management and faster workspace builds.
- Preserved architectural layering and module isolation.

## 6. Acceptance

- `cargo metadata --no-deps --format-version 1` lists exactly `repin-core` and `repin`.
- `repin-core` has no dependency on `repin`.
- `cargo test --workspace` passes with all tests intact.
- `mdbook build docs/code` and `mdbook build docs/usage` pass cleanly.
