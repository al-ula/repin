# ADR-027: CLI flag overrides for per-invocation behavior tuning

```text
Status: accepted contract and capability decision
Date: 2026-08-21
Decision type: CLI ergonomics, IPC request contracts, and configuration precedence
Builds on: ADR-021, ADR-015
```

## Decision

Repin exposes a bounded set of behavior-tuning configuration keys as per-invocation CLI flags. Flags override the merged `config.toml` value **for a single command** and never persist. They travel to the daemon over the existing IPC protocol as explicit request fields, because the daemon is a separate process that loads its own configuration.

1. **Authorized flag overrides** (added in this decision):
   - `repin search --mode <text|regex|graph|hybrid>` — overrides `retrieval.default_mode`.
   - `repin search --boost <f64>` — overrides `retrieval.centrality_boost` (applies to hybrid rank fusion).
   - `repin context --padding-lines <usize>` — overrides `context.padding_lines`.
   - `repin context --no-blast-radius` — sets `context.include_blast_radius = false`.
   - `repin context --no-verbatim-source` — sets `context.include_verbatim_source = false`.
   - `repin rerank --top-n <usize>` — overrides `intelligence.rerank.top_n` (caps candidates sent to the callback).
   - `repin rerank --deadline-ms <u64>` — overrides `intelligence.rerank.deadline_ms` (enforced on the shell callback).
   - `repin daemon run --idle-timeout <secs>` — overrides `daemon.idle_timeout_secs`.

2. **Already overridable** (documented, unchanged): `search --limit` (`retrieval.default_limit`), `rerank --agent-cmd` (`intelligence.rerank.agent_cmd`), `context --budget` (`context.default_token_budget`), `watch --interval` (`daemon.watch_debounce_ms`).

3. **Excluded from flag exposure**: structural and secret-bearing keys remain file-only — `project.*`, `storage.*`, `indexing.*`, `intelligence.providers`, `intelligence.*.endpoint`, `intelligence.*.api_key_env`. Secrets must never appear on a command line.

4. **IPC contract extension**: `IpcRequest` variants `SearchHybrid`, `Context`, `Rerank`, and `IndexAll` gain optional override fields. All fields are `Option<…>`; when `None`, the daemon uses its own configuration. Fields are additive, preserving backward compatibility within the current `PROTOCOL_MAX`.

## Rationale

ADR-021 already established "Explicit CLI Arguments / API Request Overrides" as the highest precedence layer, but only four behaviors were wired. Operators repeatedly need to tune retrieval centrality, context packing, rerank budget, and daemon lifetime for a single invocation without editing and revalidating `config.toml`.

## Consequences

- `repin-protocol` `IpcRequest` gains optional override fields on four variants.
- `repin-runtime` engine methods accept optional override parameters forwarded to ranking, context, rerank, and indexing paths.
- `repin-daemon` `handle_request` reads the override fields and forwards them.
- `repin-cli` adds the `clap` arguments and threads them into the IPC request.
- `repin-packs` / `repin-indexing` consume `tree_sitter_fallback` semantics where defined by ADR-013; until that wiring lands, no flag is exposed for it to avoid misleading no-ops.

## Required Implementation Validation

1. `cargo test -p repin-protocol` verifies IPC serde round-trip with and without override fields.
2. `cargo test -p repin-cli` verifies flag parsing and that `config show` is unaffected by flag invocation.
3. `cargo build` across the workspace plus `mdbook build docs/code` and `mdbook build docs/usage`.

## Reopen Triggers

Reopen if indexing exclusion flags (`indexing.*`) or model-selection overrides (`intelligence.*.model`) are promoted to CLI exposure, or if `tree_sitter_fallback` (ADR-013) gains a concrete fallback extractor.
