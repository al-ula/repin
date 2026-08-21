# Agent and Host Integration

Repin can be used as a CLI by an agent or embedded as a Rust library by a host application.

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

For model-assisted reranking, configure `intelligence.rerank.agent_cmd` or pass an explicit callback:

```bash
repin rerank "sqlite transaction ownership" --agent-cmd 'my-agent-rerank --json'
```

The callback protocol and safety boundaries are defined in [Optional Intelligence](../code/intelligence.md) and the [multi-tier provider specification](../code/specifications/multi-tier-model-providers.md).

## Host applications

Use the reusable runtime and capability crates when the host needs an in-process engine. The [Public API](../code/api.md) describes the stable client surface, and [Host Integration](../code/host-integration.md) describes lifecycle, freshness, capability negotiation, and change notification.

The repository includes a deterministic embedded RAG example at [`crates/repin-runtime/examples/embedded_rag.rs`](../../crates/repin-runtime/examples/embedded_rag.rs).
