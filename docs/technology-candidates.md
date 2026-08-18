# Technology Candidates

Provisional implementation choices to evaluate before Repin's implementation plan is finalized. This document is **non-normative**: the contracts and invariants in the architecture documents remain authoritative.

```text
Status: proposed
Lifecycle stage: planning
Decision point: plan finalization, after experiments
```

## 1. Current direction

| Concern | Candidate | Current status | Intended role |
|---|---|---|---|
| Implementation language | Rust | proposed | Engine and first-party adapters |
| Initial client | CLI with clap | proposed | Thin project-bound client and first interface |
| Local IPC | Linux pathname Unix-domain sockets | proposed | Central per-user daemon rendezvous for the initial PoC |
| Daemon singleton | OS-backed lease in the private runtime directory | proposed | Exactly one on-demand daemon per OS user |
| Project registry | Canonical `.repin/graph.redb` path | proposed | Active context lookup; no separate `ProjectId` |
| Authoritative store | redb | candidate | Graph facts, metadata, revisions, and recovery state |
| Lexical index | Tantivy | candidate | Rebuildable lexical and symbol retrieval |
| Vector index | USearch | candidate, deferred | Optional rebuildable semantic retrieval |
| Parsing substrate | tree-sitter + versioned queries | candidate | In-process concrete syntax parsing and declarative extraction |
| Filesystem access | cap-std + ignore + globset | candidate | Root-confined access, VCS-ignore-aware traversal, and compiled selection patterns |
| Content identity | BLAKE3 | candidate | Fast deduplication and cache hashes, not entity identity |
| Regex search | regex-automata or regex | candidate | Bounded direct regex matching with exact spans |
| VCS integration | gix or bounded Git subprocess | candidate | Startup changed-set and branch-state detection |
| File watching | notify | candidate, deferred | Platform backends behind the `Watch` port |
| Writer exclusion | fs4 or platform lock adapter | candidate | Atomic inter-process writer ownership with diagnostic metadata |
| Serialization | serde | candidate | Internal/config/protocol encoding without exposing storage types |
| Diagnostics | tracing | candidate | Structured, redaction-aware instrumentation |
| Property testing | proptest | candidate | Generated convergence, identity, and coalescing sequences |
| Snapshot/CLI testing | insta + assert_cmd | candidate | Reviewable graph snapshots and black-box CLI behavior |
| Fuzzing | cargo-fuzz + libFuzzer | candidate | Parsers, ranges, path handling, regex/query inputs |
| Dependency policy | cargo-deny + cargo-audit | candidate | License, source, duplicate, advisory, and supply-chain checks |
| Benchmarking | Criterion + iai-callgrind | candidate | Statistical end-to-end and deterministic microbenchmarks |

These are candidates, not dependencies of the core architecture. Storage acceptance requires evidence from [Storage Adapter Experiments](experiments/storage.md); extraction and operational tooling require the experiments in [Rust Foundation Experiments](experiments/rust-foundation.md). Every blocking choice requires an explicit decision during plan finalization.

USearch is not on the critical path for deterministic implementation. It may remain undecided until the embeddings milestone.

## 2. Current platform scope

| Scope | Platform | Validation and support commitment |
|---|---|---|
| Current PoC | Linux x86_64, glibc | Sole development and qualification target until the fully featured PoC is complete. Runs the complete experiment matrix, port conformance, convergence, adversarial tests, fixed-corpus benchmarks, crash/recovery experiments, and release-artifact checks. |

macOS, Windows, Linux musl/static builds, and additional architectures are not
current implementation or release targets. They become a post-PoC platform
expansion phase only after the Linux PoC is fully featured and its deterministic
implementation profile is finalized. Their minimum versions, artifact formats,
signing policy, and platform-specific behavior remain future work; no current
experiment may claim support for them from Linux evidence.

### Rust toolchain and dependency baseline

Experiments use the current stable Rust toolchain and Rust 2024 edition. They do not claim that current stable is the eventual minimum supported Rust version (MSRV). Plan finalization sets an explicit MSRV from accepted dependencies, Linux PoC evidence, and the cost of supporting older compilers.

Workspace and spike lockfiles are committed so results can be reproduced. Routine dependency updates are reviewed as a monthly batch; security or correctness updates may happen immediately. Every retained result records the exact toolchain, lockfile checksum, source pins, and feature flags. A dependency update that can affect persisted data, extraction output, ranking, platform support, or performance invalidates the applicable evidence until rerun.

## 3. Fixed architectural constraints

Experiments may change the product selections, but they do not relax these constraints:

- The working tree is authoritative for current file content.
- The `Store` is authoritative for persisted graph facts and metadata.
- Lexical and vector indexes are derived, revisioned, replaceable, and rebuildable.
- Core logic depends on port contracts, not redb, Tantivy, USearch, or their native types.
- Direct retrieval works without any index.
- Semantic retrieval is optional and cannot delay deterministic revisions.
- Exactly one authoritative writer owns a project graph; the global daemon holds
  the project lock, and inability to acquire it produces observer/direct-only
  mode with an explicit `PROJECT_LEASE_UNAVAILABLE` status.
- Deleting `.repin` is a safe rebuild/reset only after its active project
  context has unloaded; active identity changes fail the context closed.

## 4. Candidate composition

A possible Rust implementation would keep product dependencies at the adapter boundary:

```text
CLI client
  └── project selector/initializer ── pathname Unix socket
                                      │
user daemon (same binary, detached on demand)
  ├── daemon lease + bounded connection acceptor
  ├── canonical database-path context registry
  └── per-project context
       ├── Store port   ── redb adapter
       ├── Lexical port ── Tantivy adapter
       └── Vector port  ── USearch adapter (optional)
```

No redb table type, Tantivy query type, or USearch vector type may cross into the core domain or public API.

The daemon is the composition root for normal operation. Its private runtime
directory contains the central socket and singleton lease; each context owns
its project's `.repin/writer.lock`, store, watcher, and derived indexes. The
registry key is the canonical database path. The daemon records filesystem
identity only to reject an active symlink, rename, bind-mount, or alternate
spelling alias; a copied database at another canonical path is independent.

The initial IPC candidate is a pathname Unix-domain socket on Linux
x86_64/glibc. Startup uses detached same-binary candidates racing for the
per-user lease. Connection framing, admission, request concurrency, progress,
deadlines, and cancellation are bounded. Remote transport and federation are
later deployment work, not a reason to make the initial daemon multi-project
protocol depend on network reachability.

A provisional state layout is:

```text
.repin/
  graph.redb
  lexical/
  vector/
  writer.lock
```

The layout is not yet a compatibility contract. Clients never read it directly.

## 5. Why these candidates

### Rust and CLI

Rust is a plausible fit for a standalone, portable, deterministic engine with
explicit ownership of resources and a path to a single distributable binary.
The CLI is a thin project-bound client; the same binary also provides the
detached user-daemon entrypoint and its composition root. `Engine` construction
remains available internally and in tests, while remote transport is reserved
for later work.

Questions still requiring evidence include supported-platform builds, native dependency implications, parser bindings, cancellation strategy, and whether the first implementation should be synchronous or use an async runtime. The default hypothesis is a synchronous core with explicit cancellation/deadline checks and bounded worker pools; an async runtime should be added only if an experiment demonstrates a concrete need rather than as a baseline dependency.

### redb

#### S-001 experiment pin

The S1 storage spike is pinned to `redb` **4.1.0**, the unyanked crates.io release published from upstream tag [`v4.1.0`](https://github.com/cberner/redb/tree/v4.1.0) at commit [`6ed1f981ba4deab0b2adbdd7bccb46ec409b2191`](https://github.com/cberner/redb/commit/6ed1f981ba4deab0b2adbdd7bccb46ec409b2191). The exact registry archive SHA-256 is `8e925444704b5f17d32bf42f5b6e2df050bceebc3dcd6e71cc73dafe8092e839`.

| Pin property | S1 value |
|---|---|
| Dependency declaration | `redb = { version = "=4.1.0" }` |
| License | `MIT OR Apache-2.0` |
| Declared Rust version | Rust 1.89 |
| Crate edition | Rust 2024 |
| Initial optional features | None |
| Available optional features | `logging` (adds `log`); `cache_metrics` |
| Source | [crates.io release metadata](https://crates.io/api/v1/crates/redb/4.1.0) and [upstream release](https://github.com/cberner/redb/releases/tag/v4.1.0) |

S1 MUST run first with no optional redb features. A separate, explicitly labelled measurement may enable `cache_metrics`; `logging` remains disabled unless an experiment demonstrates that it is needed. The spike commits its own lockfile and records its SHA-256 in every result. This is an evidence pin only: it neither accepts redb nor selects the final MSRV. A change to this pin or any enabled feature invalidates affected S1 evidence and requires a rerun.

redb is a candidate embedded transactional store. It appears aligned with durable transactions and local deployment, but must prove the access patterns Repin actually needs rather than generic key-value throughput:

- snapshot readers and atomic batched commits
- node lookup by id, name, and owning file
- efficient outgoing and incoming edge traversal
- efficient replacement of all facts owned by one file
- durability and recovery across reopen
- writer exclusivity and observable observer/direct-only fallback
- version records and scoped migration/rebuild state

Reverse edges and file ownership indexes must be stored explicitly if deriving them would require scans.

### Tantivy

Tantivy is a candidate derived lexical index. It must prove:

- batched add and delete-by-stable-key updates
- filters required by the `Lexical` port
- stable ranking for identical index state and query
- actionable match regions that map correctly to source evidence
- commit, reopen, lag detection, repair, and rebuild behavior
- bounded behavior for supported phrase, prefix, and regex modes

Unsupported query modes may be reported through capability negotiation, but required evidence locations may not be omitted.

#### S-007 experiment pin

The S2 lexical spike is pinned to `tantivy` **0.26.1**, the unyanked crates.io release published from upstream tag [`0.26.1`](https://github.com/quickwit-oss/tantivy/tree/0.26.1) at commit [`d8f4c0b703120ed98f06297724dc1522df6019b9`](https://github.com/quickwit-oss/tantivy/commit/d8f4c0b703120ed98f06297724dc1522df6019b9). The exact registry archive SHA-256 is `edde6a10743fff00a4e1a8c9ef020bf5f3cbad301b7d2d39f2b07f123c4eac07`.

| Pin property | S2 value |
|---|---|
| Dependency declaration | `tantivy = { version = "=0.26.1", default-features = false, features = ["mmap"] }` |
| License | `MIT` |
| Declared Rust version | Rust 1.86 |
| Crate edition | Rust 2021 |
| Initial features | `mmap` only, which pulls `fs4`, `tempfile`, and `memmap2` |
| Deliberately disabled | `stemmer`/`rust-stemmers`, `stopwords`, `lz4-compression`, `columnar-zstd-compression`, `quickwit`, `failpoints` |
| Notable transitive surface | `tantivy-query-grammar` 0.26.0, `tantivy-tokenizer-api` 0.7, `tantivy-fst` 0.5, `regex` 1.5.5+ with `std`+`unicode` |
| Source | [crates.io release metadata](https://crates.io/api/v1/crates/tantivy/0.26.1) and [upstream repository](https://github.com/quickwit-oss/tantivy) |

The disabled features are not arbitrary. English stemming and stop words are actively wrong for code identifiers: stemming conflates distinct symbols and stop-word removal can delete a real identifier. Compression features change on-disk size and merge cost, so they are measured separately rather than silently included in baseline numbers. `failpoints` is a test-only facility; if S2 uses it for interruption testing, that run is labelled separately because it changes the built artifact.

Two pin consequences must be recorded rather than assumed away:

- **License differs from redb.** Tantivy is `MIT` only, not `MIT OR Apache-2.0`. This is compatible with a permissive distribution but is an input to the `cargo-deny` policy in `Q-006`, not something S2 may decide.
- **`mmap` is chosen deliberately.** Memory-mapped access interacts with the filesystem, file-locking, and fault-injection work in `S-006` and `S-005`. If the mmap path proves unusable under injected faults, the alternative is a different Tantivy directory implementation, recorded as an experiment finding rather than a contract change.

This is an evidence pin only: it neither accepts Tantivy nor selects Repin's MSRV. Note that Tantivy's declared Rust 1.86 is *lower* than redb's 1.89, so neither candidate alone determines the eventual MSRV. Changing the pin or the feature set invalidates affected S2 evidence and requires a rerun.

#### S-006 writer-lock candidate pin

Writer exclusion is evaluated through `fs4` **1.1.0**, upstream tag [`1.1.0`](https://github.com/al8n/fs4/tree/1.1.0) at commit [`5f81cf254d3e1db4a98a9a49723b7405a4d3e383`](https://github.com/al8n/fs4/commit/5f81cf254d3e1db4a98a9a49723b7405a4d3e383), registry archive SHA-256 `7e72ed92b67c146290f88e9c89d60ca163ea417a446f61ffd7b72df3e7f1dfd5`, licensed `MIT OR Apache-2.0`, declared Rust 1.75.0, edition 2021, using only the default `sync` feature.

`fs4` is also a transitive dependency of the pinned Tantivy `mmap` feature, at a different major version (Tantivy depends on `fs4` 0.13.1). The spike records whether two `fs4` majors coexist in one binary; a duplicate-version finding belongs in the `Q-006` policy, and it must not be resolved by loosening Repin's own pin to match Tantivy's.

### USearch

USearch is a candidate optional vector index. **Its evaluation (S3) is deferred**; see [S-012 deferral decision](#s-012-usearch-deferral). The uncertainties below are recorded so the experiment can be executed unchanged when the trigger fires. They are more important than raw nearest-neighbor speed:

- whether deletion actually prevents deleted entries from being returned
- persistence and crash recovery
- metadata filtering by root, language, artifact class, and node kind
- bounded over-fetching if filtering must happen outside the index
- concurrent read/update behavior
- dimension and metric enforcement
- Rust binding maturity and native build/distribution cost

Failure to meet the port contract means replacing the adapter candidate, not weakening the contract.

#### S-012 USearch deferral

S3 is **deferred**, not cancelled and not silently skipped.

Rationale: vector search serves only optional semantic retrieval. Every deterministic capability — extraction, graph queries, traversal, direct search, and lexical retrieval — is specified to work with no `Vector` adapter present, and [Optional Intelligence](intelligence.md) already requires that absence degrade only semantic features. Spending Stage 2 effort on a native-binding vector index before the deterministic core has evidence would invert that priority. The `Vector` port contract stays in place so that deferral costs nothing architecturally.

| Deferral property | Value |
|---|---|
| Status | deferred at planning; no S3 evidence will exist at Stage 2 exit |
| What is deferred | execution only; the S3 questions, cases, measurements, and pass conditions remain specified |
| Reopen trigger | semantic or hybrid retrieval entering an implementation milestone, or any requirement that depends on nearest-neighbor search |
| Also reopens if | a deterministic capability is proposed that needs vector search, which would first require revisiting the optionality decision itself |
| Blocking status | non-blocking for Stage 1 exit, Stage 2 exit, and Stage 3 candidate acceptance for `Store` and `Lexical` |
| Required at reopen | run S3 unchanged against the same `Vector` contract, and evaluate at least one alternative from the shortlist below in the same round |

Until the trigger fires, no shipped capability, benchmark claim, or release artifact may depend on a vector index, and no document may describe USearch as accepted.

#### S-013 vector candidate shortlist

S3 must not be a single-candidate evaluation, because a sole candidate makes "reject" indistinguishable from "replan." At reopen, S3 evaluates USearch plus at least one alternative against the identical `Vector` contract and the same cases. Recorded starting shortlist:

| Candidate | Shape | Why it is on the list | Primary risk to test |
|---|---|---|---|
| USearch | native library with Rust binding | compact index, HNSW, in-process, no server | binding maturity, native build/distribution during post-PoC platform expansion, deletion semantics |
| `hnsw_rs` (or an equivalent pure-Rust HNSW crate) | pure Rust | removes native toolchain and cross-compilation cost; simplest distribution story | persistence/reopen support, deletion behavior, filtering, maturity |
| Brute-force exact search over stored vectors | in-house, no dependency | exact recall by construction, trivially correct deletion, zero new supply chain | latency and memory at the Large normal band; may be adequate for early semantic use |

The brute-force row is a real candidate, not a placeholder: if exact search is fast enough at the initial target scale, it removes an entire dependency and its failure modes. Any candidate accepted later must pass the same deletion, reopen, filtering, and rebuild cases; a candidate is never accepted for speed alone while failing the deletion contract. Selecting a replacement does not relax the port — a candidate that cannot honor acknowledged deletion is rejected regardless of its recall or latency.

## 6. Supporting Rust candidates

### tree-sitter and query files

tree-sitter is the leading parsing substrate because it supports error-tolerant parsing, byte ranges, and declarative query captures. It does not by itself provide semantic resolution, stable entity identity, or trustworthy character columns. The experiment must verify grammar pinning, query compatibility, cancellation behavior, invalid UTF-8 handling, and byte-to-character range conversion. Grammar and query versions participate in extractor invalidation.

### Discovery, identity, and watching

`cap-std` is a candidate for opening paths relative to pre-opened root directory capabilities, reducing ambient path access and TOCTOU exposure. The `ignore` crate is a candidate for efficient `.gitignore`-aware discovery, while `globset` can compile engine selection and exclusion patterns. The experiment must verify how discovery paths are reopened through root capabilities on every supported platform. Canonicalization alone is insufficient if a symlink can be swapped before open, and library defaults are not the selection policy.

BLAKE3 is a candidate for file content identity, deduplication, and cache keys. Content hashes MUST NOT participate in node identity ([Graph Model §4](graph-model.md#4-identity)).

`regex` offers linear-time matching for its supported syntax; `regex-automata` exposes lower-level configuration useful for compile and memory bounds. The direct-search experiment should select the smallest API that provides exact spans, explicit syntax, cancellation safe points, and enforced limits. Tantivy regex support is a lexical capability and must not define direct regex semantics.

The VCS adapter needs a separate experiment. `gix` avoids invoking an executable and can operate through Rust APIs, but has a larger dependency/semantic surface. A bounded `git` subprocess may track users' Git behavior more closely but adds executable discovery, environment sanitization, output limits, cancellation, and version variance. Neither is accepted by assumption.

`notify` is the likely first watch adapter, deferred until I3. Its events are untrusted hints: startup scans, normalization, coalescing, content-hash deduplication, overflow escalation, and periodic reconciliation remain engine responsibilities.

Project writer exclusion should use an OS-backed advisory lock through a small
adapter (`fs4` is one candidate), with a separate diagnostic metadata record.
This per-project lock is distinct from the daemon singleton lease. The
experiment must verify release on crash, contention behavior, filesystem
support, and the policy for filesystems where reliable locks are unavailable.
PID/staleness heuristics alone are rejected.

### Quality and supply-chain tooling

- `proptest` should generate seeded change sequences for convergence, order-independence, identity, and watcher coalescing tests.
- `insta` is a candidate for reviewable normalized graph snapshots; snapshot updates still require semantic review. `assert_cmd` can exercise CLI exit codes, stdout/stderr separation, and JSON stability without coupling tests to CLI internals.
- `cargo-fuzz` should target parser inputs, byte/character range conversion, path normalization, redaction, and query parsing. Fuzz corpora and crash cases become fixtures.
- `cargo-deny` should enforce allowed licenses, approved registries/git sources, duplicate-version policy, and dependency bans. `cargo-audit` covers RustSec advisories; generated SBOMs and locked dependencies belong in release planning.
- Criterion is appropriate for noisy end-to-end timing distributions. `iai-callgrind` is a candidate for stable instruction-count regressions in deterministic hot paths. Neither replaces fixed-corpus wall-clock benchmarks.
- `serde` and `tracing` are enabling dependencies, but protocol/config schemas and redaction policy remain owned by Repin rather than inferred from those libraries.

## 7. Cross-index model to validate

redb and Tantivy cannot be assumed to share one transaction. The proposed recovery model is:

1. Commit authoritative graph facts and the new graph revision in the store.
2. Record derived-index work or enough source state to reconstruct it.
3. Apply and commit the lexical update.
4. Record the lexical revision as complete.
5. Process vector work asynchronously and advance its independent revision.

After interruption, revision mismatch must be detectable. The engine repairs or rebuilds a lagging derived index. A lexical or vector hit is validated against the authoritative graph before return; a hit for a missing node is dropped.

The exact write-ahead or pending-work representation is intentionally undecided until the combined recovery experiment.

## 8. Decision gates

During plan finalization, each candidate receives one outcome:

- **accepted** — becomes part of the initial implementation profile
- **rejected** — replaced, with the reason and experiment evidence retained
- **deferred** — not needed for the next implementation milestone

Rust, redb, Tantivy, the parsing substrate, file discovery, content hashing, serialization, and baseline test/security tooling are blocking choices before deterministic implementation begins. Watching and USearch are non-blocking until their implementation milestones.

Acceptance requires:

1. Applicable port conformance behaviors pass in the spike.
2. Failure and recovery behavior is demonstrated, not inferred.
3. Measurements use the method in [Conformance §6](conformance.md#6-benchmark-method).
4. Build and distribution constraints are documented for target platforms.
5. No adapter requires product-specific behavior above L0.
6. Remaining limitations have an explicit capability or degradation behavior.

Accepted choices should be recorded as ADRs under `docs/decisions/` during plan finalization. This proposed document remains the candidate record until then.

## 9. Open questions

- Which minimum Linux distribution/kernel/glibc, macOS, and Windows versions are supportable according to experiment evidence?
- Which local filesystem semantics are required, and what is the explicit behavior on network, virtual, or unusual filesystems?
- Which archive/installer formats and signing policy apply to each release artifact?
- Does redb need an external lock record in addition to its own writer behavior?
- What pending-work protocol gives lexical repair the simplest crash semantics?
- Can Tantivy produce precise regions for every advertised query mode?
- Can USearch satisfy metadata filtering directly, or is bounded post-filtering sufficient?
- Does USearch introduce unacceptable native build or static-linking constraints?
- What latency/throughput targets define acceptable performance for the initial workstation corpus bands?
- Which of the Rust, Markdown, TypeScript, and JavaScript experiment packs are included in the first deterministic implementation?
- Can pinned tree-sitter grammars and queries produce deterministic facts across supported targets?
- How will byte offsets be converted to Unicode-scalar columns without repeated whole-file scans?
- Which regex engine and syntax satisfy bounded direct-search behavior?
- Can capability-based filesystem access satisfy containment and discovery behavior across the support matrix?
- Does the first VCS adapter use `gix`, a bounded Git subprocess, or another implementation?
- Is a synchronous core sufficient for cancellation, watching, and future service adapters?
- What evidence-based MSRV and dependency-source policy will be guaranteed at plan finalization?
