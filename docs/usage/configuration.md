# Configuration

Repin uses TOML configuration. The CLI can create, display, and validate it:

```bash
repin config init
repin config init --global
repin config show
repin config validate
```

`config init` creates project configuration at `<root>/.repin/config.toml`. `config init --global` creates user configuration at `~/.config/repin/config.toml`. Existing files are preserved unless `--force` is supplied.

## Discovery

The effective configuration is assembled from built-in defaults, the user configuration, and one project configuration layer. Repin uses the following project file selection:

1. An explicit `--config <PATH>` file, when supplied.
2. `<root>/.repin/config.toml`, when present.
3. `<root>/config.toml`, when the metadata configuration is absent.

Later layers override earlier values. `repin config show` prints the merged result that the CLI will use.

## Common settings

The starter template documents every supported section. These defaults are useful reference points:

| Section | Default |
| --- | --- |
| `[indexing]` | Respect `.gitignore`, index documentation and configuration, and skip files larger than 2 MiB. |
| `[retrieval]` | Hybrid search with a limit of 50 results. |
| `[context]` | An 8192-token default budget with two padding lines around source ranges. |
| `[daemon]` | 150 ms watch debounce and a 3600-second idle timeout. |
| `[intelligence.*]` | Disabled until a provider is selected. |

A small project configuration can look like this:

```toml
schema_version = 1

[project]
name = "my-repository"
roots = ["."]

[indexing]
exclude_paths = ["generated/**", "vendor/**"]
max_file_size_bytes = 4194304

[retrieval]
default_mode = "hybrid"
default_limit = 50
```

Use `repin config validate` after editing. Invalid schema versions, escaping roots, and unsafe project-level credential settings are rejected.

## CLI flag overrides

A bounded set of behavior keys may be overridden per invocation through CLI flags without editing `config.toml`. Flags never persist; they apply only to the single command. The following keys accept overrides (see the [CLI Reference](cli.md) for flag names):

| Config key | Flag |
| --- | --- |
| `retrieval.default_mode` | `repin search --mode` |
| `retrieval.centrality_boost` | `repin search --boost` |
| `retrieval.default_limit` | `repin search --limit` |
| `context.padding_lines` | `repin context --padding-lines` |
| `context.include_blast_radius` | `repin context --no-blast-radius` |
| `context.include_verbatim_source` | `repin context --no-verbatim-source` |
| `context.default_token_budget` | `repin context --budget` |
| `intelligence.rerank.top_n` | `repin rerank --top-n` |
| `intelligence.rerank.deadline_ms` | `repin rerank --deadline-ms` |
| `intelligence.rerank.agent_cmd` | `repin rerank --agent-cmd` |
| `daemon.idle_timeout_secs` | `repin daemon run --idle-timeout` |
| `daemon.watch_debounce_ms` | `repin watch --interval` |

Structural and secret-bearing keys (`project.*`, `storage.*`, `indexing.*`, `intelligence.providers`, `*.endpoint`, `*.api_key_env`) remain file-only and are intentionally not exposed as flags.

## Provider credentials

Project configuration may select a provider, but provider profiles and `api_key_env` entries belong in the user-global file. Keep secrets in environment variables; do not place secret values in TOML.

The supported capability tiers are:

- `embedded` for local ONNX models;
- `agent` for a shell callback, such as reranking;
- configured remote providers such as OpenAI, Ollama, or Google.

The full schema, precedence rules, and safety floors are specified in [Per-Project Configuration](../code/specifications/project-configuration.md).
