# Embedded RAG proof

The `repin-runtime` example `embedded_rag` demonstrates the library topology
without importing the daemon or CLI:

```text
repository snapshots
  -> repin_core::indexing
  -> lexical/graph retrieval and optional exact vector candidates
  -> repin_core::context graph expansion and deterministic budget packing
  -> caller-owned inference
```

Run the deterministic proof offline:

```text
cargo test -p repin-runtime --example embedded_rag
```

Run the example itself:

```text
cargo run -p repin-runtime --example embedded_rag
```

The example uses a fake embedding model and caller-owned inference, keeps
SQLite optional by using an in-memory store in the proof, and prints the
context's provenance-relevant paths and truncation state. It has no daemon or
CLI dependency. The graph-free direct path remains available through
`repin-direct-search` and the `SourceFs` contract.

An actual local-provider smoke is opt-in and requires a cached model (or
explicitly allowing the adapter's model download):

```text
cargo run -p repin-runtime --example embedded_rag -- --local-model <model-id>
```

This command is deliberately outside the default and CI paths. Credentials,
network access, model assets, latency, and hardware are recorded by the smoke
run rather than hidden in deterministic tests.
