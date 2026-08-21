# Research Record: redb + Tantivy versus SQLite + FTS5

```text
Status: concluded research record backing ADR-009
Date: 2026-08-19
Method: secondary research and comparative analysis
Decision outcome: SQLite + FTS5 accepted for authoritative persistence; redb + Tantivy preserved as fallback
```

## Question

Which persistence profile provides the optimal initial implementation choice for Repin?

- **Profile A:** redb 4.1.0 as the authoritative store plus Tantivy 0.26.1 as a separate lexical index.
- **Profile B:** SQLite through rusqlite 0.40.1, with FTS5 in the same database as the authoritative graph facts.

The subject of comparison is the complete profile: graph storage, lexical retrieval, commit and recovery behavior, Rust integration, operational state, and degradation paths.

Statements below are tagged by evidence type:

- **Documented** — verified by upstream documentation or source.
- **Derived** — follows from documented behavior and Repin's contracts.
- **Reported** — a published measurement whose workload is not Repin's.
- **Unknown** — requires implementation-time workload validation.

## Conclusion and decision basis

Select **SQLite + FTS5 for the initial Linux PoC** ([ADR-009](../decisions/ADR-009-sqlite-fts5-initial-profile.md)). Keep redb + Tantivy as a documented fallback if implementation evidence indicates that FTS5 cannot satisfy required lexical precision or workload budgets.

The primary architectural advantage is transaction coupling: SQLite commits graph revisions and FTS5 index mutations in the exact same transaction. Profile A requires coordinating two independently committed stores, maintaining durable intent, acknowledgement, lag detection, and asynchronous repair state machines. Eliminating that cross-store state machine removes substantial correctness risk.

A follow-up analysis of [libSQL in embedded-local mode](libsql-embedded-local.md) confirmed that libSQL preserves SQLite's transaction coupling but introduces asynchronous API wrappers and maintenance uncertainties without improving I0/I1 requirements.

## Contract comparison

| Repin concern | redb + Tantivy | SQLite + FTS5 | Assessment |
| --- | --- | --- | --- |
| Atomic graph updates | redb write transactions commit or abort atomically. | Ordinary tables participate in SQLite transactions. | Both meet. |
| Snapshot readers with one writer | redb permits concurrent read transactions and one write transaction. | WAL mode permits readers concurrent with one writer; a read transaction holds a snapshot. | Both meet. |
| Acknowledged commit and recovery | redb exposes durability modes and repairs after an unclean shutdown. | SQLite documents atomic commit and recovery; WAL durability depends on `synchronous`. | Both meet with an explicit durability configuration. |
| Required graph lookups and reverse edges | Requires an application-designed table, multimap, and composite-key layout. | Requires relational tables and explicit indexes, including separate source and target edge indexes. | Both meet through schema design; SQLite is easier to inspect and migrate. |
| Conditional base revision | Read and compare the revision inside the sole redb write transaction, then commit or abort. | Check or conditionally update the revision inside the SQLite write transaction. | Both meet through the adapter. |
| Durable bounded change history | Application tables and retention logic. | Application tables and retention logic. | Equal; neither product supplies the Repin abstraction. |
| Graph and lexical atomicity | Impossible across redb and Tantivy; requires pending intent, lexical commit, acknowledgement, detection, and repair. | Graph rows and a normal FTS5 table can be changed in one SQLite transaction. | SQLite materially simpler. |
| Incremental lexical replacement | Tantivy supports delete-by-term and add before commit. There is no native primary key. | FTS5 supports `INSERT`, `UPDATE`, and `DELETE`; its virtual table cannot declare a primary key, so the adapter owns the stable key-to-rowid mapping. | Both meet through the adapter. |
| Terms, phrase, prefix, ranking, snippets | Tantivy supports these directly and offers per-field tokenizers. | FTS5 supports term, phrase, prefix, Boolean/NEAR queries, BM25, highlight, and snippets; tokenizer configuration is table-wide. | Both meet the initial negotiated capabilities. Tantivy is richer. |
| Regex lexical mode | Tantivy regex operates over indexed terms, not arbitrary raw text. | FTS5 has no standard regex query mode. | Neither is equivalent to Repin's direct raw-text regex. Both advertise `regex=false` and use direct-scan fallback. |
| Metadata filtering | Boolean queries and fast fields can filter Tantivy candidates. | Indexed ordinary columns can filter or join FTS5 results in SQL. | Both meet through schema/query design. |
| Exact source byte regions | Tantivy tokenizers produce byte offsets, but its inverted positions are token ordinals; snippets rescan stored document text. | FTS5 instance APIs expose token offsets, not byte offsets; its tokenizer callback receives byte offsets. | Both require adapter-level mapping or re-scan. Repin re-reads and verifies current source bytes in either profile. |
| Deterministic result order | Tantivy's default equal-score tie-break uses internal document addresses. | FTS5 BM25 does not provide Repin's stable-key tie-break by itself. | Both require explicit application tie-breaking. |
| Bounded cancellation | No general search-interrupt API in Tantivy; custom collector or worker pool required. | SQLite exposes progress handlers and `sqlite3_interrupt()`; rusqlite exposes an interrupt handle. | SQLite has the clearer documented mechanism. |
| File-format and migration horizon | redb and Tantivy have both required format migrations across releases. | SQLite documents long-term format stability; schema migrations can be transactional. | SQLite has the stronger maintenance record. |
| Build and dependency shape | Pure-Rust top-level stores, but two persistence libraries and cross-index protocols. | One transactional engine behind a C FFI; bundled rusqlite compiles pinned SQLite. | Different risk shapes. SQLite reduces runtime state. |

## Accepted SQLite implementation profile

The finalized implementation profile under [ADR-009](../decisions/ADR-009-sqlite-fts5-initial-profile.md):

- `rusqlite = 0.40.1` with `default-features = false` and `features = ["bundled", "hooks"]`;
- Bundled SQLite 3.53.2 from `libsqlite3-sys` 0.38.1 with FTS5 enabled;
- `.repin/graph.sqlite3` as the canonical project database path;
- WAL mode, `synchronous=FULL`, foreign keys enabled, and bounded busy handling;
- One coordinated writer connection and separate bounded reader connections;
- Normalized graph tables and explicit indexes for all lookup/traversal paths;
- A normal content-bearing FTS5 table with `detail=full` in the same database;
- Stable `DocKey` mapped to FTS5 rowid;
- Deterministic stable-key tie-breaking; and
- Working-tree re-read and exact region verification before evidence exposure.

## Preserved fallback profile (redb + Tantivy)

If implementation benchmarks reveal unresolvable limitations in SQLite/FTS5, the repository preserves the following evaluated fallback pins:

- `redb = { version = "=4.1.0" }`
- `tantivy = { version = "=0.26.1", default-features = false, features = ["mmap"] }`
- `fs4 = { version = "=1.1.0", features = ["sync"] }`

## Sources

- redb [`Database`](https://docs.rs/redb/latest/redb/struct.Database.html), [`ReadTransaction`](https://docs.rs/redb/latest/redb/struct.ReadTransaction.html), [`WriteTransaction`](https://docs.rs/redb/latest/redb/struct.WriteTransaction.html)
- Tantivy [`IndexWriter`](https://docs.rs/tantivy/latest/tantivy/indexer/struct.IndexWriter.html), [Architecture](https://docs.rs/crate/tantivy/latest/source/ARCHITECTURE.md), [`TopDocs`](https://docs.rs/tantivy/latest/tantivy/collector/struct.TopDocs.html)
- SQLite [Isolation](https://www.sqlite.org/isolation.html), [WAL](https://www.sqlite.org/wal.html), [`synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous), [FTS5](https://www.sqlite.org/fts5.html), [Interrupt](https://www.sqlite.org/c3ref/interrupt.html)
- rusqlite [0.40.1 Documentation](https://docs.rs/rusqlite/latest/rusqlite/) and [`libsqlite3-sys`](https://docs.rs/crate/libsqlite3-sys/latest/source/build.rs)
