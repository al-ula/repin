# Specification — Per-Project Configuration System (`config.toml`)

This normative specification defines the syntax, schema semantics, discovery rules, and safety invariants for Repin repository configuration files.

---

## 1. Discovery and Precedence

The engine resolves effective configuration by merging multiple layers in order of increasing precedence:

```text
1. Engine Built-in Conservative Defaults
2. User Global Configuration (~/.config/repin/config.toml)
3. Project Metadata Configuration (<root>/.repin/config.toml)
4. Project Root Configuration (<root>/config.toml)
5. Explicit CLI Flags / IPC Request Overrides
```

### Discovery Algorithm
1. If the caller explicitly provides `--config <PATH>` (`-c <PATH>`), that path is loaded directly. If it does not exist, an error is returned.
2. Otherwise, check `<root>/.repin/config.toml`. If present and valid, load it as the project configuration layer.
3. If not found in `.repin/`, check `<root>/config.toml`. If present and valid, load it as the project configuration layer.
4. If neither exists, project configuration is treated as empty (`RepinConfig::default()`). No I/O errors are raised.

---

## 2. Schema Specification

Configuration files use TOML syntax with UTF-8 encoding. All tables and keys are optional unless specified otherwise.

```toml
schema_version = 1

[project]
# name = "my-repository"
# roots = ["."]
# languages = ["rust", "typescript"]

[indexing]
exclude_paths = [
  "**/build/**",
  "**/dist/**",
  "vendor/**"
]
exclude_extensions = ["min.js", "bundle.js"]
max_file_size_bytes = 2097152
respect_gitignore = true
index_docs = true
index_config = true

[extraction]
tree_sitter_fallback = true

[retrieval]
default_mode = "hybrid"
default_limit = 50
centrality_boost = 0.15
regex_size_limit_bytes = 10485760

[context]
default_token_budget = 8192
padding_lines = 2
include_blast_radius = true
include_verbatim_source = true

[storage]
wal_checkpoint_mode = "truncate"
checkpoint_interval = 1000

[daemon]
watch_debounce_ms = 150
idle_timeout_secs = 3600

# ==============================================================================
# Optional Model-Powered Intelligence (Default: none / disabled)
# ==============================================================================

# --- Example Tier 1: Embedded Local ONNX (Zero external network / API keys) ---
# [intelligence.embedding]
# provider = "embedded"
# model = "Alibaba-NLP/gte-modernbert-base"
# dimension = 256
# auto_download = true
#
# [intelligence.rerank]
# provider = "embedded"
# model = "Alibaba-NLP/gte-reranker-modernbert-base"
# top_n = 50
# deadline_ms = 100

# --- Example Tier 2: Agent-Powered (IDE Agents & Shell Callbacks) ---
# [intelligence.rerank]
# provider = "agent"
# agent_cmd = "my-agent-rerank --json"
# deadline_ms = 5000

# --- Example Tier 3: Standard APIs (OpenAI, Ollama, Google Gemini) ---
# [intelligence.providers.openai]
# endpoint = "https://api.openai.com/v1"
# api_key_env = "OPENAI_API_KEY"
#
# [intelligence.providers.ollama]
# endpoint = "http://localhost:11434"
#
# [intelligence.providers.google]
# endpoint = "https://generativelanguage.googleapis.com"
# api_key_env = "GEMINI_API_KEY"
#
# [intelligence.embedding]
# provider = "openai"
# model = "text-embedding-3-small"
# dimension = 256
#
# [intelligence.enrichment]
# provider = "google"
# model = "gemini-2.5-flash"
```

---

## 3. Schema Fields and Semantics

### `schema_version` (Integer, Required)
Specifies the configuration schema format version. Must be `1` for the current specification.

### `[project]`
- `name` (String, Optional): Logical identifier for the repository.
- `roots` (List of Strings, Optional): Relative paths within the repository considered project roots (default: `["."]`). Must not escape root.
- `languages` (List of Strings, Optional): Explicit language priority hints.

### `[indexing]`
- `exclude_paths` (List of Strings, Optional): Glob patterns for files or directories to skip during scanning and indexing.
- `exclude_extensions` (List of Strings, Optional): File extensions without leading dots to exclude from scanning.
- `max_file_size_bytes` (Integer, Optional): Upper file size limit in bytes for extraction and indexing (default: `2097152` / 2 MB).
- `respect_gitignore` (Boolean, Optional): Whether to load and respect `.gitignore` rules (default: `true`).
- `index_docs` (Boolean, Optional): Whether to index documentation and prose files into the graph and FTS5 (default: `true`).
- `index_config` (Boolean, Optional): Whether to extract configuration manifests into `config_key` entities (default: `true`).

### `[extraction]`
- `tree_sitter_fallback` (Boolean, Optional): Whether to fall back to Tree-sitter parsers if native grammars are unavailable (default: `true`).

### `[retrieval]`
- `default_mode` (String, Optional): Default search mode when unspecified (`"hybrid"`, `"graph"`, `"direct"`, or `"regex"`; default: `"hybrid"`).
- `default_limit` (Integer, Optional): Default candidate limit for search queries (default: `50`).
- `centrality_boost` (Float, Optional): Degree centrality weight in deterministic rank fusion (default: `0.15`).
- `regex_size_limit_bytes` (Integer, Optional): Maximum compiled regex buffer size (default: `10485760` / 10 MB).

### `[context]`
- `default_token_budget` (Integer, Optional): Default token budget for context construction (default: `8192`).
- `padding_lines` (Integer, Optional): Surrounding source lines included with symbol definitions (default: `2`).
- `include_blast_radius` (Boolean, Optional): Pack caller and dependency counts into context headers (default: `true`).
- `include_verbatim_source` (Boolean, Optional): Include 1-indexed source code slices (default: `true`).

### `[storage]`
- `wal_checkpoint_mode` (String, Optional): SQLite WAL checkpoint mode on batch write completion (`"truncate"`, `"passive"`, `"full"`; default: `"truncate"`).
- `checkpoint_interval` (Integer, Optional): Write operation count before triggering checkpoint (default: `1000`).

### `[daemon]`
- `watch_debounce_ms` (Integer, Optional): Filesystem watcher debounce duration in milliseconds (default: `150`).
- `idle_timeout_secs` (Integer, Optional): Idle timeout before releasing memory locks (default: `3600`; `0` for persistent).

### `[intelligence]`
- `[intelligence.providers.<name>]` (Table, Optional): Shared provider profiles defining `endpoint` (URL) and `api_key_env` (Environment variable name).
- `embedding` (Table, Optional): Semantic vector search configuration (`provider`, `model`, `dimension`, `auto_download`, `endpoint`, `api_key_env`; default: `provider = "none"`).
- `rerank` (Table, Optional): Reranking configuration (`provider`, `model`, `top_n`, `deadline_ms`, `agent_cmd`; default: `provider = "none"`).
- `enrichment` (Table, Optional): Graph enrichment configuration (`provider`, `model`, `api_key_env`; default: `provider = "none"`).

---

## 4. Safety Floors and Security Invariants

1. **Immutable Exclusions**: Hardcoded safety exclusions (`.git`, `.repin`, `.env`, `.env.*`, `id_rsa*`, `*.pem`, `*.key`) are merged via **set union**. A project configuration cannot override or remove these entries.
2. **Root Containment**: All relative paths (`roots`, `exclude_paths`) are validated using capability-relative filesystem semantics (`cap-std`). Path traversal sequences (`../`) that escape the repository root are rejected.
3. **No Code Execution on Parse**: Loading `config.toml` is strictly a pure data deserialization step. Callback shell commands (`agent_cmd`) are never executed automatically on configuration load.
4. **Global-Only Credential Isolation**: Project-level configurations (`<root>/.repin/config.toml`, `<root>/config.toml`) are rejected by `repin config validate` if they attempt to define `[intelligence.providers]` or `api_key_env`. All provider endpoints and API credential references reside exclusively in the User Global Configuration (`~/.config/repin/config.toml`).

---

## 5. CLI Configuration Management

Repin provides built-in commands to initialize, inspect, and validate configuration files:

```bash
# Initialize project-level configuration (.repin/config.toml) with full tier examples
repin config init

# Initialize user global configuration (~/.config/repin/config.toml) with full tier examples
repin config init --global

# Force overwrite existing configuration
repin config init --force

# Display effective merged configuration (Global + Project + Defaults)
repin config show

# Validate syntax, schema version, root containment, and safety floors
repin config validate
```

Both project-level and user-global initialization generate the complete starter template containing commented-out, ready-to-uncomment configuration blocks for:
1. **Tier 1: Embedded Local ONNX** (`gte-modernbert-base`, `gte-reranker-modernbert-base`)
2. **Tier 2: Agent-Powered Pipelines** (`agent_cmd` with JSON-RPC over stdin/stdout)
3. **Tier 3: Standard APIs & Shared Profiles** (`openai`, `ollama`, `google` with `api_key_env`)

