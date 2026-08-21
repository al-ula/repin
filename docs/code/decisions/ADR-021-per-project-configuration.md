# ADR-021: Per-project configuration file and precedence merge protocol

```text
Status: accepted contract and capability decision
Date: 2026-08-20
Decision type: system configuration, precedence algebra, safety floor enforcement, and CLI ergonomics
Builds on: ADR-003, ADR-010, ADR-013, ADR-017, ADR-018, ADR-019, ADR-020
```

## Decision

Repin adopts a deterministic, hierarchical configuration system based on TOML (`config.toml`):

1. **Discovery Hierarchy & Precedence**:
   Configuration merges with strict precedence (lowest to highest):
   ```text
   Engine Conservative Defaults
     < User Global Configuration (~/.config/repin/config.toml)
     < Project Configuration (.repin/config.toml or ./config.toml)
     < Explicit CLI Arguments / API Request Overrides
   ```
   - If both `.repin/config.toml` and `./config.toml` exist, `.repin/config.toml` takes precedence.
   - Indexless operation is guaranteed: if no configuration file is present, the engine defaults to conservative internal values with zero required I/O.

2. **Schema & Versioning**:
   - The top-level document includes `schema_version = 1`.
   - Sections cover `[project]`, `[indexing]`, `[extraction]`, `[retrieval]`, `[context]`, `[storage]`, `[daemon]`, and `[intelligence]`.
   - Partial configurations merge cleanly over defaults without requiring all tables or fields to be specified.
   - Unknown fields produce non-fatal diagnostic warnings.

3. **Immutable Safety Floors**:
   - **Root Containment**: Project configuration cannot point to or read paths outside the project root.
   - **Secret Exclusions**: Hardcoded security exclusions (`.git`, `.repin`, `.env`, `id_rsa`, `*.pem`, `*.key`) are merged via set union and can never be disabled by project configuration.
   - **No Arbitrary Execution**: External callback commands (e.g. `intelligence.rerank.agent_cmd`) require host trust and explicit invocation.

4. **CLI Ergonomics & Subcommands**:
   - Add `repin config init` to generate a starter `.repin/config.toml`.
   - Add `repin config show` to display the active merged configuration with provenance.
   - Add `repin config validate` to perform syntax and schema checks.
   - Add global `--config <PATH>` (`-c <PATH>`) flag to override discovery.
   - CLI commands (`repin search`, `repin context`, `repin index`, `repin watch`) use configured defaults when explicit flags are omitted.

## Rationale

Without a standardized per-project configuration file:
- Repositories with custom build directories (`build/`, `dist/`, `vendor/`) required repetitive command-line exclusion flags.
- Retrieval tuning (e.g. result limits, graph centrality boost) and context packing budgets could not be customized per codebase.
- Daemon watcher debounce timings could not be tuned for high-latency or monorepo environments.

Using TOML matches Rust workspace idioms (`Cargo.toml`) and allows human-readable, typed, and structured configuration.

## Consequences

- `repin-core` defines the typed configuration models (`RepinConfig`, `ProjectConfig`, `IndexingConfig`, `RetrievalConfig`, etc.) and the `Merge` trait.
- `repin-fs` updates `ExclusionFilter` to ingest `IndexingConfig` while maintaining hardcoded safety baselines.
- `repin-engine` consumes `RepinConfig` for rank fusion, context formatting, and extraction options.
- `repin-cli` introduces `repin config` subcommands, global `-c/--config` flag, and integrates defaults into CLI execution.
- `repin-daemon` loads and applies watcher debounce and timeout configurations.

## Required Implementation Validation

1. `cargo test -p repin-core` verifies serialization, deserialization, default generation, and hierarchical merging.
2. `cargo test -p repin-fs` verifies that custom exclusion patterns merge with immutable safety floors and cannot expose `.env` or sensitive keys.
3. `cargo test -p repin-engine` verifies retrieval and context formatting honoring configured limits.
4. `cargo test -p repin-cli` verifies CLI flag overrides, `repin config` subcommands, and discovery paths.
5. `cargo test -p repin-conformance` validates the end-to-end configuration test matrix.

## Reopen Triggers

Reopen this decision if multi-root workspace hierarchies require complex inheritance across nested repository directories beyond the primary project root.
