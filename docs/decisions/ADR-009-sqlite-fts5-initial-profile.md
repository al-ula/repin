# ADR-009: Use SQLite with FTS5 for the initial persistence profile

```text
Status: accepted implementation choice for the initial Linux PoC
Date: 2026-08-19
Decision type: authoritative store and lexical adapter
Supersedes: proposed redb + Tantivy direction in technology-candidates.md
```

## Decision

Repin's initial implementation profile uses one local SQLite database for the
authoritative graph store and lexical index:

- `rusqlite = 0.40.1`, with `default-features = false` and the `bundled`
  feature, plus `hooks` where needed for bounded progress instrumentation;
- bundled SQLite 3.53.2 from `libsqlite3-sys` 0.38.1;
- `.repin/graph.sqlite3` as the canonical project database path;
- WAL mode, `synchronous=FULL`, foreign keys enabled, and bounded busy
  handling;
- one coordinated writer connection with bounded concurrent reader
  connections;
- normalized graph tables and explicit indexes for every Store-port lookup and
  traversal path;
- a normal content-bearing FTS5 table in the same database and transaction
  domain as the graph;
- a stable application `DocKey` mapped to the FTS5 rowid;
- explicit stable-key tie-breaking above native lexical score; and
- current working-tree re-read and exact region verification before lexical
  evidence is returned.

The Store and Lexical ports remain separate contracts. They are implemented by
one SQLite adapter boundary, so the core does not depend on rusqlite, SQLite,
FTS5, or SQL types.

Regex remains a direct bounded scan and is advertised as absent from the FTS5
capability set. Vector search remains deferred under ADR-007. ADR-012 selects
an exact Rust scan over derived SQLite embedding rows for I5; semantic writes
still use a later transaction and separate revision/recovery protocol.

## Evidence

The [redb + Tantivy versus SQLite + FTS5 research](../research/redb-tantivy-vs-sqlite.md)
compared the complete profiles against Repin's Store and Lexical contracts.
The [libSQL embedded-local follow-up](../research/libsql-embedded-local.md)
confirmed that libSQL retains the same graph-to-FTS transaction advantage but
does not improve the current I0/I1 requirements enough to justify its larger
native/API and maintenance surface. Public SQLite documentation establishes
WAL snapshots, one-writer behavior, atomic transactions, FTS5 capabilities,
interrupt support, and long-term file-format compatibility.

No Repin-specific performance claim is implied. The profile is accepted from
desk research and theoretical consistency analysis; implementation conformance
and workload measurements follow as validation tasks.

## Consequences

- Graph facts, revision metadata, change history, and lexical document changes
  can commit or roll back together. The graph-to-lexical pending-intent and
  acknowledgement protocol is removed from the initial profile.
- The project has one transaction domain, but WAL still creates `-wal` and
  `-shm` sidecars while active. Discovery, backup, cleanup, and diagnostics
  must account for them.
- FTS5's table-wide tokenizer, lack of standard raw-text regex, and lack of a
  direct byte-range result remain adapter responsibilities. Exact source
  ranges are derived or verified against current bytes.
- The profile adds a bundled C build and FFI boundary. The release profile
  must inventory the compiler, native source, feature flags, and licenses.
- The existing redb/Tantivy candidate work remains useful as a fallback if
  SQLite/FTS5 cannot satisfy a required lexical or resource constraint, but it
  is not part of the initial dependency set.
- Native libSQL vector search is deferred rather than rejected. It may be
  reconsidered at I5 if transaction-coupled vector state materially changes
  the recovery design.

## Required implementation validation

The following validate the selected profile; they do not reopen the decision
merely because they are still outstanding:

1. Store-port schema and conformance: owner/file removal, bidirectional edges,
   conditional revision commits, durable bounded change history, migrations,
   and newer-schema refusal.
2. SQLite behavior: rollback, reopen, WAL checkpointing, writer contention,
   integrity checks, backup/restore, and process-crash recovery with the
   selected durability setting.
3. Lexical behavior: FTS5 updates in the same transaction, filters, supported
   term/phrase/prefix modes, deterministic ordering, Unicode-correct exact
   regions, and stale-index verification.
4. Cancellation and deadlines across the SQLite adapter and the synchronous
   core.
5. Fixed-corpus resource measurements for bulk updates, graph traversal,
   filtered lexical search, cold reopen, database size, and WAL growth.
6. Bundled build, lockfile, toolchain, license, and native-component inventory.

## Reopen triggers

Reopen ADR-009 if implementation evidence shows that:

- a required Store or Lexical behavior cannot be met without weakening its
  port contract;
- exact regions require an unbounded corpus scan;
- cancellation cannot meet the eventual deadline;
- the fixed-corpus resource budget fails after schema/query-plan tuning; or
- the bundled native profile conflicts with the accepted release policy.

If only lexical capability is the problem, compare SQLite for the graph plus a
separate Tantivy index before returning to redb. If vector retrieval is the
reason, evaluate libSQL's local vector implementation as a separate I5
decision.

## Not decided

This ADR does not select the final parser, regex engine, VCS adapter, watcher,
vector implementation, production platform matrix, final MSRV, or release
artifact/signing policy.
