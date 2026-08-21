# ADR-024: Compatibility versioning and conservative state replacement

```text
Status: accepted contract decision
Date: 2026-08-20
Builds on: ADR-009, ADR-015, ADR-023
Backs: docs/storage.md, docs/runtime.md, docs/api.md, docs/results.md,
       docs/conformance.md
```

## 1. Context

Repin has several independent compatibility boundaries. A package rebuild can
change executable provenance without changing the IPC protocol, the SQLite
schema, or the facts produced by a language pack. Treating those dimensions as
one version causes unnecessary daemon replacement, unsafe store activation, or
over-broad graph invalidation. The reusable crate topology in ADR-023 also
requires compatibility authorities to live in the crates that own them.

## 2. Decision

Repin adopts these independent authorities:

| Boundary | Authority | Compatibility use |
| --- | --- | --- |
| Package/API | each Cargo package's `CARGO_PKG_VERSION` | diagnostics and published API identity |
| IPC | `repin-protocol` protocol range | bootstrap negotiation and domain framing |
| SQLite store | `repin-store-sqlite` format/application ID and schema version | inspect, open, migrate, or reject |
| Semantic facts | core registries, resolution/classification rules, packs, and extractors | scoped invalidation |
| Build provenance | CLI/release metadata | diagnostics only |

The owning crates expose checked-in constants for protocol, store, and core
semantic versions. Build commit and build-ID metadata is optional and never
participates in compatibility decisions. Ordinary local builds must succeed
without either value. Reusable crates do not read a downstream project's VCS
environment.

The CLI binary identity is `v<package>-<commit>`, where `<package>` is the
Cargo package version and `<commit>` is the first 12 characters of the full
Git commit supplied through `REPIN_GIT_COMMIT`. Builds without provenance use
`v<package>-unknown`. `repin --version` and the JSON `version` field use this
identity. `repin version --json` also reports the decomposed package, full
commit, and build fields plus all compatibility dimensions; absent optional
provenance remains JSON `null`.

## 3. Bootstrap and daemon replacement

IPC begins with a bounded, versioned bootstrap envelope. It carries the
client's protocol range and diagnostic identity; it does not carry project
selection or acquire a project writer lock. Project binding and store access
occur only after the ranges overlap. The highest common protocol version is
selected deterministically. Malformed, unsupported, or oversized bootstrap
messages receive a stable `PROTOCOL_MISMATCH` result or are closed according
to the framing contract; no unknown-protocol shutdown command is sent.

The initial local framing profile bounds every frame at 1 MiB, bounds the
bootstrap frame at 64 KiB, and applies a 2,000 ms bootstrap read deadline.
Frames exceeding their bound are rejected before JSON/domain decoding and the
connection is closed without mutating project state.

Bootstrap rejection includes the daemon's supported range, package/build
diagnostics, and whether a replacement request is eligible. A replacement
retry is encoded as another bootstrap envelope, not as an unknown domain
command. The daemon accepts it only when the client range is strictly newer
than the daemon range and the full-idle predicate holds; a strictly older
client can never replace the daemon.

Overlapping protocol ranges continue normally even when package or build
identities differ. A compatible daemon remains in service. An incompatible
client may request replacement only after the daemon reports that it is fully
idle: no attached connections other than the requesting bootstrap connection,
no active contexts, no in-flight or recovery work, and no authoritative commit.
The daemon acknowledges the request, waits for that connection to close, then
exits. A busy daemon remains alive and returns actionable `PROTOCOL_MISMATCH`.
An older client never replaces or downgrades a newer incompatible daemon.
Concurrent candidates use the singleton daemon lease and successor readiness;
losers reconnect to the elected winner. Automatic draining of a busy daemon is
deferred and requires a separate decision.

## 4. SQLite identity and evolution

The SQLite adapter identifies its format with `PRAGMA application_id` and its
physical schema with `PRAGMA user_version`. Inspection reads both values and
the serialized `VersionRecords` before any schema DDL, cleanup, or graph
activation. The adopted application ID is `0x5250_494E` (`RPIN`) and the
portable diagnostic format ID is `repin.sqlite`.

An empty newly-created database is `(application_id, user_version) = (0, 0)`.
Creation stamps the application ID and schema version in the same transaction
as the initial schema. Opening an existing database classifies it as current,
older supported, older unsupported, newer, unrelated, or corrupt/contradictory.
Only an authorized migration may change `user_version`, and that change is in
the final successful transaction. A newer schema returns `PROJECT_STATE_NEWER`;
unknown format, corruption, and disagreement between `user_version` and
`VersionRecords.store_schema_version` return `PROJECT_STATE_INVALID`.
Existing legacy Repin databases require an explicit migration or rebuild path;
opening one must not silently run `CREATE TABLE IF NOT EXISTS` as cleanup.

The current physical schema is version 2. Version 2 adds the
`migration_journal(id, from_version, to_version, completed_at)` table used for
auditable migration completion. The supported migration is v1→v2: it creates
that table and updates `PRAGMA user_version` and
`VersionRecords.store_schema_version` in one transaction. If any statement or
version-record decode fails, the transaction rolls back and the v1 database
remains unchanged. No migration is attempted for a newer or contradictory
database.

## 5. Durable semantic versions and invalidation

`VersionRecords` contains store schema, kind registry, attribute registry,
classification, resolution, pack, extractor, engine, VCS, and dirty-set
fields. Version records commit atomically with the replacement facts or
metadata they describe. A failed replacement leaves the prior authoritative
revision and its prior records visible.

The coordinator uses producer/file enumeration and bounded resolution-input
enumeration to make invalidation complete: registry changes follow their
adopted migration/rebuild rule; classification reclassifies affected files
without parsing when persisted data permits it; resolution replaces all
resolution-derived claims without source reads; pack changes re-extract pack
owned files; and extractor changes replace only claims from that extractor and
version. `removeClaims(owner)` alone is not a sufficient discovery mechanism.

## 6. Consequences and validation gate

Compatibility errors use the existing public vocabulary: `PROTOCOL_MISMATCH`,
`PROJECT_STATE_NEWER`, `PROJECT_STATE_INVALID`, `PROJECT_LEASE_UNAVAILABLE`,
`DAEMON_START_FAILED`, and `DAEMON_UNAVAILABLE`. Direct retrieval remains
available when graph activation fails safely.

Implementation may begin only after the normative updates in the backed
documents compile with `mdbook build`. Conformance must cover protocol range
selection, bootstrap bounds, store classification and transactional migration,
version-record atomicity, scoped invalidation, and conservative replacement.
