# Reusable-library extraction baseline

This record is the M1 checkpoint for the ADR-023 extraction. It fixes the
commands, fixture shape, and invariants used to compare the extracted crates
with the original in-process engine composition.

## Fixture and semantic baseline

The baseline fixture is the existing in-process engine coverage:

- a graph-free repository containing `src/main.rs`, queried through direct
  retrieval;
- a graph-backed repository containing `src/lib.rs` and `compute_sum`, indexed
  through the default language packs and queried through graph retrieval; and
- deterministic ranking, vector, context, filesystem containment, provider
  absence, and JSON-RPC agent fixtures in their respective crate tests.

The clean pre-extraction workspace passed:

```text
cargo test --workspace
```

The same command remains the correctness and compatibility gate after every
crate movement. Canonical ordering is asserted by the ranking, traversal,
vector, context, and facade tests; the stable tie-break is the opaque node or
edge identity.

## Reproducible review commands

Run these commands from the repository root on the pinned Linux PoC toolchain:

```text
cargo test --workspace
cargo test -p repin-runtime --example embedded_rag
cargo test -p repin-runtime --test conformance_tests
cargo clippy -p repin-core --all-targets -- -D warnings
cargo metadata --no-deps --format-version 1
mdbook build docs/code
```

For a local timing sample, keep the fixture, build profile, filesystem, and
warm/cold state fixed and record the complete command output:

```text
/usr/bin/time -p cargo test --workspace
/usr/bin/time -p cargo run -p repin-runtime --example embedded_rag
```

The first command measures the complete correctness suite. The embedded proof
exercises source walking, transactional indexing, lexical/graph retrieval,
exact vector retrieval, context packing, and caller-owned inference in one
repeatable path.

## Provider smoke record

The offline provider gate records the following deterministic outcomes:

- agent JSON-RPC reranking succeeds for a structured response;
- malformed agent output returns a provider error;
- an agent deadline returns a timeout; and
- an absent embedded model and absent API credential remain explicit capability
  failures without network access.

The live remote-provider smoke is opt-in because it requires operator-owned
credentials and an endpoint. The adapter command is exercised with a configured
provider through the runtime, and its endpoint, model, latency, and result are
recorded with the timing sample when that smoke is run.

## I/O and allocation invariants

The extraction review checks these invariants against the pre-extraction
operation paths:

- direct retrieval opens no graph store and performs only the selected source
  walk;
- capability crates communicate through borrowed port contracts and add no
  in-process JSON or other serialization boundary;
- runtime hybrid retrieval performs one read view and one lexical query, with
  candidate reads through that view;
- context construction reads each selected source snapshot through the source
  contract and applies deterministic deduplication; and
- the facade delegates to the runtime without an extra store round trip.

Any future allocation or latency regression measurement must use the same
fixture and report median, p95, run count, hardware, filesystem, and warm/cold
state. ADR-023 accepts up to 5% after variance analysis and requires zero new
serialization boundaries, source reads, or store round trips in an existing
operation.

This baseline is a semantic and structural checkpoint for the current PoC;
large-corpus throughput qualification remains a separate benchmark activity
under [Conformance §6](../conformance.md#6-benchmark-method).
