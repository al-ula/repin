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

## Provider credentials

Project configuration may select a provider, but provider profiles and `api_key_env` entries belong in the user-global file. Keep secrets in environment variables; do not place secret values in TOML.

The supported capability tiers are:

- `embedded` for local ONNX models;
- `agent` for a shell callback, such as reranking;
- configured remote providers such as OpenAI, Ollama, or Google.

The full schema, precedence rules, and safety floors are specified in [Per-Project Configuration](../code/specifications/project-configuration.md).
