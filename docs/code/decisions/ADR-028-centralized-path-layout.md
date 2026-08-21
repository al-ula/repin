# ADR-028: Centralized product layout and ownership

```text
Status: accepted
Date: 2026-08-21
Decision type: filesystem layout, host boundary, and library-crate ownership
Builds on: ADR-003, ADR-021, ADR-023, ADR-026
```

## 1. Problem

Concrete Repin paths are currently constructed in several crates:

- project state: `.repin`, `graph.sqlite3`, `writer.lock`, `.gitignore`, and
  `config.toml`;
- user configuration: `~/.config/repin/config.toml`;
- daemon rendezvous: `daemon.sock`, `daemon.lock`, and the runtime directory;
- model cache location: the user cache root and `models`.

The duplication permits layout drift. It also makes reusable crates aware of
host defaults. Current examples include configuration discovery in
`repin-runtime`, `HOME` lookup in `repin-intelligence`, and repeated daemon
socket construction in the CLI.

Configuration ownership has a second form of drift: the CLI resolves an
explicit configuration path, while the daemon handshake carries only a
database path. A path selected by `--config` therefore cannot configure the
daemon context.

## 2. Decision

Add a small product-level workspace crate named `repin-product`. It owns
Repin-specific path names, layout constructors, and host-default selection.
Layout constructors are pure. The crate performs no filesystem I/O,
canonicalization, permission change, or directory creation. Its environment
helpers only select explicit host bases such as `HOME`, `XDG_RUNTIME_DIR`, and
the process temporary directory.

`repin-product` is deliberately outside the generic dependency tree. It has no
workspace-crate dependencies. It is shared by the product entry points, not
advertised as a generic capability crate:

```text
other-project ──> generic Repin crates

repin-cli ───────┐
                 ├─> repin-product (std only)
repin-daemon ────┘

generic Repin crates ──> no repin-product dependency
```

The arrows show dependency direction. Generic crates, including
`repin-runtime` and `repin-intelligence`, never import `repin-product` or a
product layout type. They accept ordinary `Path`/`PathBuf` values, explicit
roots, or resolved configuration values.

The crate exposes typed layouts for project, runtime, and user scopes. The user
layout carries the product model-cache root:

```text
ProjectLayout(root)
  state_dir       = root/.repin
  database        = root/.repin/graph.sqlite3
  project_config  = root/.repin/config.toml
  root_config     = root/config.toml
  compatibility   = root/repin.toml
  writer_lock     = root/.repin/writer.lock
  ignore_marker   = root/.repin/.gitignore

RuntimeLayout(base)
  socket          = base/daemon.sock
  daemon_lock     = base/daemon.lock

UserLayout(config_base, cache_base)
  global_config   = config_base/repin/config.toml
  model_root      = cache_base/repin/models

```

The exact Rust representation uses typed layout structs and namespaced host
selectors. The product path literals live in one implementation location.

## 3. Ownership rules

| Responsibility | Owner | Rule |
|---|---|---|
| Product path names and joins | `repin-product` | Product-level construction only |
| `HOME`, XDG, temporary, or platform directory selection | `repin-product` | Resolve environment once, then pass explicit bases |
| Project discovery and explicit `--config` selection | CLI or host adapter | Produce paths through `repin-product` |
| Daemon socket and singleton lease I/O | `repin-daemon` | Open and mutate paths supplied by its runtime layout |
| Project state lifecycle I/O | `repin-daemon` | Create, validate, and remove the project layout |
| Configuration file I/O | Configuration loader | Read bytes, parse with `repin-core`, and produce `RepinConfig` |
| Runtime behavior | `repin-runtime` | Consume resolved configuration; perform no config-file discovery |
| Model cache base selection | CLI or host composition | Produce the base with `repin-product` and pass it to the provider |
| Model asset filenames and cache I/O | Intelligence provider adapter | Own provider-format names such as `model.onnx`; consume an explicit `Path`; perform no `HOME` lookup or product-layout dependency |
| SQLite I/O | `repin-store-sqlite` | Consume an explicit database path |
| Repository source I/O | `repin-fs` | Continue using capability-relative access |

`repin-core` continues to own typed configuration values and TOML parsing. It
does not own product filesystem paths or file-loading errors. `repin-runtime`
receives a resolved `RepinConfig`; it does not discover configuration files.

## 4. Configuration flow

The resolved configuration becomes an input to the daemon context. The chosen
flow is:

```text
CLI/host path selection
  -> configuration loader
  -> RepinConfig
  -> project activation / IPC request
  -> daemon context
  -> runtime
```

The runtime receives `RepinConfig` or an equivalent resolved configuration
object. It does not inspect `config.toml` or `repin.toml` itself.

The existing physical configuration locations remain stable:

1. user global configuration;
2. `<root>/.repin/config.toml`;
3. `<root>/config.toml`;
4. explicit caller/API overrides.

`repin.toml` remains a compatibility alias only if an explicit deprecation
decision retains it. Its literal and precedence must be centralized with the
other path names during the transition.

## 5. Model-cache flow

The cache base directory is selected by the composition root and passed to the
embedded provider as an ordinary `Path`. `repin-intelligence` owns its
provider-specific asset filenames and joins them beneath that caller-owned
root. `list_cached_models` and model-management operations receive the same
explicit root rather than reconstructing a user path.

`LocalModelAssets::config_path` remains valid terminology: it identifies the
model's `config.json`, not Repin application configuration.

## 6. Migration sequence

1. Add `repin-product` and layout tests without changing physical paths.
2. Replace daemon constants and socket/lease joins.
3. Replace CLI project, configuration, runtime-directory, and model-cache joins.
4. Move configuration loading out of `repin-runtime` and pass the resolved
   configuration into daemon/runtime activation.
5. Pass an explicit model-cache base into `repin-intelligence`.
6. Remove duplicate path constants and legacy path construction.
7. Decide separately whether to retain or deprecate `repin.toml`.

The migration preserves `.repin` contents, database identity, writer-lock
ownership, socket naming, and model-cache locations. No store-schema or IPC
version bump is required for path construction alone. An additive IPC field for
resolved configuration remains within the current protocol range under
ADR-024.

## 7. Safety and acceptance criteria

- `repin-product` has no workspace dependencies. Its layout constructors have
  no filesystem I/O and its host selectors are the only environment readers.
- All product path construction literals occur in `repin-product` or normative
  documentation. CLI help may repeat documented paths as user-facing text.
- Generic library crates have no dependency on `repin-product` and receive
  explicit bases or resolved configuration objects.
- Runtime construction creates no configuration directories and reads no
  configuration files.
- Model-cache tests pass with an injected temporary base and without `HOME`.
- Existing state discovery, initialization, daemon election, and CLI behavior
  remain unchanged.
- `cargo test --workspace` passes.
- `mdbook build` passes.

## 8. Compatibility follow-up

- Configuration values travel as an additive optional field in the project
  activation request. The daemon uses defaults when an older client omits it.
- Whether `repin.toml` remains as a documented compatibility alias.
