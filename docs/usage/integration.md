# Agent Integration

Repin can be used as a CLI by an agent. Library embedding is outside this guide; see `crates/repin-core/examples/embedded_rag.rs` for an in-process example.

## Install the agent skill

Install the Repin skill across your AI coding agents (Claude Code, Cursor, Codex, Pi, OpenCode, etc.) using `npx skills`:

```bash
# Global installation for all agents
npx skills add al-ula/repin -g -y

# Or install for specific agents
npx skills add al-ula/repin -g -a claude-code -a cursor
```

The skill definition lives in `skills/repin/SKILL.md` and provides guidance for symbol navigation, blast-radius impact analysis, call path tracing, and budgeted context packing.

## Agent workflows

Use bounded commands that return source-backed evidence:

```bash
repin inspect src/lib.rs
repin search -g "Runtime"
repin impact "Runtime"
repin context "Where is project state created?" --budget 32768
repin review-context --since 1
```

Use `impact --json`, `path --json`, and `db inspect --json` when a caller needs machine-readable output. Paths and budgets should be supplied by the caller rather than inferred from unbounded repository scans.

## Rerank callback

Optional model capabilities are disabled by default. For model-assisted reranking, configure `intelligence.rerank.agent_cmd` or pass an explicit callback:

```bash
repin rerank "sqlite transaction ownership" --agent-cmd 'my-agent-rerank --json'
```

The callback is a subprocess. Repin writes one JSON-RPC request to stdin and reads one JSON-RPC response from stdout. The deadline is `intelligence.rerank.deadline_ms`, overridable with `--deadline-ms`. A missing, failing, or timed-out callback leaves the deterministic ranking in place.

Input:

```json
{
  "jsonrpc": "2.0",
  "method": "repin/rerank",
  "params": {
    "query": "session eviction timer",
    "candidates": [
      { "id": "fn:reap_idle_leases", "content": "pub fn reap_idle_leases(&mut self) { ... }" },
      { "id": "fn:connect_client", "content": "pub fn connect_client(...) { ... }" }
    ]
  }
}
```

Output:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "ranked": [
      { "id": "fn:reap_idle_leases", "score": 0.94 },
      { "id": "fn:connect_client", "score": 0.12 }
    ]
  }
}
```
