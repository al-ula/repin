# Research Record: libSQL in embedded-local mode

```text
Status: concluded research record backing ADR-009 / ADR-012
Date: 2026-08-19
Method: secondary research and comparative analysis
Scope: libsql 0.9.30, local database, core feature only
Disposition: deferred; upstream SQLite accepted for initial profile
Confidence: moderate
```

## Question

Should Repin use libSQL instead of upstream SQLite for the embedded-local
graph and lexical profile?

This analysis is limited to:

```toml
libsql = { version = "=0.9.30", default-features = false, features = ["core"] }
```

and `Builder::new_local(path)`. It excludes Turso Cloud, remote queries,
embedded replicas, synchronization, and `sqld`.

## Conclusion and disposition

Do **not** replace upstream SQLite + FTS5 with libSQL for the initial Linux
PoC. Classify libSQL as a **deferred, vector-driven alternative** for potential re-evaluation
at milestone I5 ([ADR-012](../decisions/ADR-012-exact-rust-vector-baseline.md)).

For graph plus lexical storage, local libSQL provides the same
architectural advantage as SQLite (tables and FTS5 sharing a
transaction) but does not improve hard I0/I1 requirements enough to
justify:

- depending on a SQLite fork when upstream SQLite already satisfies all requirements;
- adapting the synchronous core to libSQL's asynchronous public Rust API;
- accepting a larger native build surface;
- relying on a project whose maintainers indicate primary new feature development has moved
  to the separate Turso database; and
- accepting unresolved, current safety reports against the local Rust wrapper version.

libSQL's primary local advantage is built-in vector storage and ANN
search. Vector retrieval is deferred by [ADR-007](../decisions/ADR-007-optional-capability-sequencing.md), with [ADR-012](../decisions/ADR-012-exact-rust-vector-baseline.md) selecting an exact Rust vector baseline.

## Contract and feature comparison

| Concern | Upstream SQLite through rusqlite | libSQL embedded-local | Consequence for Repin |
| --- | --- | --- | --- |
| Required graph and lexical behavior | Meets through ordinary tables and FTS5. | Meets through the same model. | No libSQL advantage for I0/I1. |
| Transaction domain | Graph and FTS5 share one SQLite transaction. | Graph and FTS5 share one libSQL transaction. | Equal architectural result. |
| Rust API shape | Synchronous connection, statement, row, and transaction API. | High-level operations and row iteration are `async`. | rusqlite matches ADR-002 synchronous core. |
| Cancellation | Safe interrupt handle and progress-handler wrapper. | Safe `Connection::interrupt()`. | Both can interrupt; rusqlite offers clearer progress wrapper. |
| FTS5 | Enabled in bundled `libsqlite3-sys` build. | Enabled in bundled `libsql-ffi`. | Equal feature availability. |
| Vector search | External index / exact Rust scan in I5. | Built-in vector types and DiskANN index. | Evaluated at I5 if exact scan misses budget. |
| Build surface | Bundled SQLite compiled by `cc`. | `libsql-ffi` requires `bindgen`, `cmake`, `cc`, `glob`. | libSQL adds supply-chain surface. |
| Wrapper maturity | Mature synchronous wrapper with long stability history. | `libsql` 0.9.30 has open local-wrapper safety reports. | Upstream SQLite has lower validation risk. |
| Maintenance horizon | Upstream SQLite long-term support through 2050. | Active maintenance with feature focus shifted to Turso. | Upstream SQLite has clearer long-term horizon. |

## Future re-evaluation conditions (Milestone I5)

libSQL should be reconsidered at milestone I5 only if:

1. exact Rust vector search misses fixed-corpus resource or latency budgets;
2. transaction-coupling vector state materially simplifies the recovery design;
3. native vector search satisfies filtering, deletion, determinism, and cancellation contracts; and
4. the Rust binding resolves upstream soundness and wrapper reports.

## Sources

- libSQL [Repository and Notice](https://github.com/tursodatabase/libsql), [`Builder::new_local`](https://docs.rs/libsql/latest/libsql/struct.Builder.html), [`Connection`](https://docs.rs/libsql/latest/libsql/struct.Connection.html)
- libSQL FFI [`libsql-ffi`](https://docs.rs/crate/libsql-ffi/latest)
- Turso [Vector Announcement](https://turso.tech/blog/turso-brings-native-vector-search-to-sqlite)
- Upstream issues: [#2251](https://github.com/tursodatabase/libsql/issues/2251), [#2257](https://github.com/tursodatabase/libsql/issues/2257), [#2233](https://github.com/tursodatabase/libsql/issues/2233), [#2212](https://github.com/tursodatabase/libsql/issues/2212)
- SQLite [LTS Horizon](https://www.sqlite.org/lts.html) and rusqlite [Documentation](https://docs.rs/rusqlite/latest/rusqlite/)
