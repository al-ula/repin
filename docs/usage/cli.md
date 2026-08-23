# CLI Reference

Run `repin --help` for the complete generated synopsis. The commands below are grouped by the task they perform.

## Installation and updates

| Command | Purpose |
| --- | --- |
| `repin install [SOURCE]` | Install Repin and bundled documentation into `~/.local/share/repin` with symlink at `~/.local/bin/repin`. |
| `repin update [--check] [--force]` | Check for or install the latest release from GitHub. |
| `repin check-update` | Check if a newer version is available from GitHub. |

## Project lifecycle

| Command | Purpose |
| --- | --- |
| `repin init [PATH]` | Create `.repin` metadata and index the repository. Add `--no-index` to defer indexing. |
| `repin uninit [PATH]` | Remove `.repin` metadata. Add `--force` or `-y` to skip confirmation. |
| `repin index` | Build the authoritative graph and derived indexes from the selected repository. |
| `repin sync` | Apply worktree changes incrementally. |
| `repin rebuild graph\|lexical\|vector\|all` | Rebuild the selected authoritative or derived index. |
| `repin status` | Show daemon connection, graph revision, and index status. |

## Search and navigation

| Command | Purpose |
| --- | --- |
| `repin search <PATTERN>` | Search using the configured default mode. |
| `repin search -r <PATTERN>` | Search the working tree with a bounded regular expression. |
| `repin search -g <PATTERN>` | Search symbol declarations in the graph. |
| `repin search --hybrid <PATTERN>` | Combine lexical and graph search. |
| `repin search <PATTERN> --mode <direct\|regex\|graph\|hybrid>` | Override `retrieval.default_mode` for this invocation. |
| `repin search <PATTERN> --boost <F64>` | Override `retrieval.centrality_boost` for hybrid ranking (this invocation). |
| `repin inspect <PATH>` | Print a file's structural outline and declared symbols. |
| `repin at-position <PATH> <LINE> <COLUMN>` | Resolve the symbol at a source coordinate. |
| `repin entity <NAME_OR_ID>` | Show entity metadata. |
| `repin neighbors <NAME_OR_ID> --max-depth <N>` | Traverse nearby graph relationships. |
| `repin impact <NAME_OR_ID> --max-depth <N>` | Analyze downstream and upstream impact. Add `--json` for a structured envelope. |
| `repin path <FROM> <TO> --max-depth <N>` | Trace a dependency or call path. Add `--json` for a structured envelope. |

## Context and intelligence

| Command | Purpose |
| --- | --- |
| `repin context <QUERY> --budget <BYTES>` | Assemble bounded context for an LLM or agent. |
| `repin context <QUERY> --padding-lines <N>` | Override `context.padding_lines` for this invocation. |
| `repin context <QUERY> --no-blast-radius` | Disable blast-radius expansion (`context.include_blast_radius = false`). |
| `repin context <QUERY> --no-verbatim-source` | Disable verbatim source (`context.include_verbatim_source = false`). |
| `repin review-context --since <REVISION> --budget <BYTES>` | Assemble context around changed files and graph impact. |
| `repin rerank <QUERY> [CANDIDATES]` | Rerank candidates with a configured or explicit agent callback. |
| `repin rerank <QUERY> --top-n <N>` | Override `intelligence.rerank.top_n` for this invocation. |
| `repin rerank <QUERY> --deadline-ms <MS>` | Override `intelligence.rerank.deadline_ms` (agent callback deadline). |
| `repin model download <MODEL>` | Download model assets into the local cache. |
| `repin model list` | List cached model assets. |
| `repin model remove <MODEL>` | Remove cached model assets. |

Optional model capabilities are disabled by default. Configure a provider before using model-backed commands; see [Configuration](configuration.md).

## Runtime and diagnostics

| Command | Purpose |
| --- | --- |
| `repin watch --interval <MILLISECONDS>` | Continuously apply worktree changes. |
| `repin daemon run` | Run the daemon in the foreground. |
| `repin daemon run --idle-timeout <SECS>` | Override `daemon.idle_timeout_secs` for this daemon process. |
| `repin daemon status` | Inspect daemon process and socket state. |
| `repin daemon stop` | Stop the background daemon. |
| `repin daemon restart` | Restart it using the selected project's database. |
| `repin stop` / `repin restart` | Short lifecycle aliases. |
| `repin db inspect [PATH]` | Inspect SQLite identity and schema without activating the graph. Add `--json` for structured output. |
| `repin db migrate [PATH]` | Apply an explicitly authorized SQLite migration. |
| `repin version [--json]` | Print `v<package>-<commit>` identity with a 12-character commit suffix and compatibility diagnostics. |
| `repin eval` | Run the retrieval evaluation suite. |

Use `--project <PATH>` when the current directory is outside the repository. Use `--config <PATH>` when the configuration file is outside the normal discovery locations.
