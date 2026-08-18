# Rust Foundation Experiments

Disposable experiments for non-storage choices in [`docs/technology-candidates.md`](../technology-candidates.md).

```text
Status: planned
Lifecycle stage: planning
Execution stage: experimentation
Production code: no
```

## 1. Shared rules

Follow the reproducibility and benchmark method in [`docs/conformance.md` §6](../conformance.md#6-benchmark-method) and [`docs/experiments/storage.md` §2](storage.md#2-shared-method). Record each run or comparable run group with [`docs/experiments/template.md`](template.md). Pin every crate, grammar, and query revision. Retain commands, fixtures, raw results, and failing seeds. A spike demonstrates behavior; it is not promoted directly into production.

Linux x86_64/glibc is the sole current experiment and qualification target.
The fully featured PoC must be completed and its deterministic implementation
profile finalized before any macOS, Windows, musl/static, or additional-
architecture implementation work begins. Those platforms are post-PoC scope;
Linux evidence must not be presented as support evidence for them.

### Preparation profile

The preparation artifacts in this document are inputs to disposable spikes, not
production code or technology decisions. A run is identified by
`(experimentId, fixtureId, candidatePins, toolchain, platform, seed)`; changing
any member invalidates the comparable result group. Every run records exact
crate, grammar, query, tool, and feature pins even when a candidate is rejected.

The current matrix runs F1–F7 completely on Linux x86_64/glibc. No
cross-platform subset or non-Linux artifact job is part of the current PoC.
After the PoC is fully featured, a separate platform-expansion plan will define
portable correctness, build, and artifact cases for each additional target.

The following rules apply to every adapter spike:

1. Library iteration order, pointer identity, thread scheduling, locale,
   filesystem enumeration order, wall-clock time, and environment variables
   MUST NOT affect an oracle or a normalized fact batch.
2. A candidate-specific failure is recorded as a bounded skip, unavailable
   capability, or rejected candidate according to the applicable contract; it
   MUST NOT be hidden by a fallback that changes meaning.
3. A result is comparable only when fixture manifests, generated oracles,
   candidate pins, build flags, and workload bounds match.

These rules close the planning gap between the normative contracts and the
experiment template without accepting any candidate dependency.

### Open-task follow-up execution profile — 2026-08-18

The disposable follow-up harness at
[`foundation_followup.rs`](foundation_spike/src/bin/foundation_followup.rs)
shares the JSON result shape used by the first foundation spike. It runs the
open tasks in dependency order:

```text
F-017 -> F-018 -> F-009 -> F-019 -> F-014 -> F-015 -> F-020
```

The final retained Linux x86_64/glibc run is
[`foundation-followup-20260818-v7`](results/raw/foundation-followup-20260818-v7/batch.json).
Its manifest records the fixed seed, candidate pins, active feature set,
source revision, lockfile SHA-256, toolchain, host environment, and exact
platform. The reproducible command from
`docs/experiments/foundation_spike` is:

```sh
cargo run --release --locked --offline \
  --features gix-adapter,sniff-adapter \
  --bin repin-foundation-followup -- \
  run-all --output ../results/raw/foundation-followup-20260818-v7
```

The normalized query/capture, path, sniff, hash/update, regex, and VCS
artifacts were repeated and compared byte-for-byte; dynamic timing samples and
Git commit IDs remain explicitly identified as non-normalized measurements.
Every report remains provisional evidence. No dependency, parser, sniffing
policy, regex adapter, VCS adapter, retry default, or production API was
selected. F-016 is intentionally absent and remains deferred until I3 watching
planning begins.

### Candidate pins for the first foundation spike

The following pins are a reproducibility baseline checked on 2026-08-18. They
are evidence pins only: the spike may reject, replace, or split them after the
Linux PoC build and behavior matrix. Registry archive checksums and the
resolved lockfile checksum are captured when the spike workspace is created.

| Area | Candidate pin | Build/runtime note |
|---|---|---|
| tree-sitter Rust binding | `tree-sitter = 0.26.11` | Rust 2021 crate; builds the bundled C runtime through a C compiler/linker; optional Wasm support is not enabled in F1 |
| Rust grammar | `tree-sitter-rust = 0.24.2` (upstream tag `v0.24.2`, release commit `77a3747`) | generated parser plus native build script; verify language ABI with the pinned core |
| Markdown grammar | `tree-sitter-md = 0.5.3` | split block/inline grammars; optional parser convenience feature is evaluated separately; the upstream project documents syntax-coverage limits |
| TypeScript/TSX grammar | `tree-sitter-typescript = 0.23.2` | two language functions (`typescript`, `tsx`); generated parser plus C build step |
| JavaScript/JSX grammar | `tree-sitter-javascript = 0.25.0` | generated parser plus C build step; JSX is part of the grammar surface |
| query files | Repin manifest `f1-query-v1` | query text, capture-role ordinals, and SHA-256 per pack; no parser-library query order is trusted |
| root capabilities | `cap-std = 4.0.2` | capability-relative filesystem APIs; verify Linux PoC behavior first, then repeat platform-specific semantics during post-PoC expansion |
| ignore traversal | `ignore = 0.4.31` | `.gitignore`/override traversal candidate; engine selection precedence remains authoritative |
| compiled selection globs | `globset = 0.4.19` | compile user/project patterns after safety-floor patterns; record case and separator settings explicitly |
| content hashing | `blake3 = 1.8.5` | default `std` feature only for the baseline; parallel hashing is a separate measurement |
| direct regex | `regex = 1.13.1` and `regex-automata = 0.4.16` | compare the safe high-level API with the lower-level bounded engines under F6; neither pin selects the final adapter |
| VCS library | `gix = 0.86.0` | evaluate trust/config behavior and feature minimization against a subprocess baseline |
| optional async runtime | `tokio = 1.53.1` | F4-only feature pin: `rt-multi-thread`, `sync`, `time`, `net`, `io-util`, and `macros`; the synchronous build has no Tokio dependency |

Sources: [tree-sitter Rust binding](https://docs.rs/crate/tree-sitter/0.26.11),
[tree-sitter release](https://github.com/tree-sitter/tree-sitter/releases/tag/v0.26.11),
[Rust grammar release](https://github.com/tree-sitter/tree-sitter-rust/releases/tag/v0.24.2),
[Markdown binding](https://docs.rs/crate/tree-sitter-md/0.5.3),
[TypeScript binding](https://docs.rs/crate/tree-sitter-typescript/0.23.2),
[JavaScript binding](https://docs.rs/crate/tree-sitter-javascript/0.25.0),
[cap-std](https://docs.rs/crate/cap-std/4.0.2),
[ignore](https://docs.rs/crate/ignore/0.4.31),
[globset](https://docs.rs/crate/globset/0.4.19),
[BLAKE3](https://docs.rs/crate/blake3/1.8.5),
[regex](https://docs.rs/crate/regex/1.13.1),
[regex-automata](https://docs.rs/crate/regex-automata/0.4.16), and
[gix](https://docs.rs/crate/gix/0.86.0). The source pages are references for
the pin; the experiment result still records the immutable archive or commit
digest actually used.

The F1 pack manifest must also record whether the Markdown binding is the
`tree-sitter-md` split parser or the older `tree-sitter-markdown` single-parser
crate. They are not interchangeable inputs: their node topology, inline parse
strategy, and compatibility with the pinned core differ. The first spike uses
`tree-sitter-md` as the primary candidate and keeps the older crate only as an
explicit comparison if fixture coverage requires it.

## 2. F1 — tree-sitter extraction substrate

Use the Rust, Markdown, TypeScript, and JavaScript fixture families and corpus bands in [`docs/experiments/fixtures.md`](fixtures.md). The first-spike grammar/query pins are recorded in the preparation profile; pinning does not accept tree-sitter or commit all four packs to the first release.

### F1 Questions

- Do pinned Rust bindings and grammar crates parse deterministically on every proposed target?
- Can declarative query files express the first language packs without per-node boundary chatter?
- How are error recovery, cancellation, time limits, and pathological nesting surfaced?
- Can byte ranges be mapped accurately to Repin's Unicode-scalar line/column contract?

### F1 Cases

1. Parse identical fixtures repeatedly and compare serialized capture batches byte-for-byte.
2. Run malformed, partial, deeply nested, huge-line, CRLF, non-ASCII, and invalid-UTF-8 fixtures.
3. Pin a grammar and query set, then intentionally change each version and verify scoped invalidation inputs.
4. Extract one code language and one prose/structured format using query captures plus bounded local processing.
5. Measure parse, query, capture materialization, range conversion, and fact construction separately.
6. Exercise parser cancellation or timeout mechanisms and verify a bounded skip result.
7. Build all grammar bindings on every proposed target and record native-toolchain requirements.

### F1 Pass conditions

- Identical versioned inputs produce identically ordered facts and ranges.
- No parser-owned handle escapes `FactBatch`.
- Error recovery produces bounded partial facts or a recorded skip.
- Ranges satisfy the line/column/byte-offset contract for Unicode, CRLF, and invalid input.
- Grammar/query upgrades are visible version inputs and produce reviewable snapshot diffs.
- Build requirements fit the support matrix.

### F1 preparation: capture order and range oracle

The adapter materializes all captures before constructing facts. It then sorts
them by this versioned tuple, in ascending bytewise order:

```text
(rootId, normalizedRelativePath, startByte, endByte,
 captureRoleOrdinal, sourceNodeKindOrdinal, normalizedName,
 extractorLocalDiscriminator)
```

`captureRoleOrdinal` comes from the pinned query/pack manifest, not from the
parser's capture enumeration. `sourceNodeKindOrdinal` and name normalization
come from the pack contract. The local discriminator is required when two
captures remain equal; it is derived from source order in the input bytes, not
from a library object address or a worker id. Identical canonical facts are
deduplicated only after this sort. A spike MUST emit the pre-dedup capture
sequence in its diagnostics so a nondeterministic adapter can be diagnosed
without weakening graph equality.

The range oracle uses the contract in [Graph Model §7](../graph-model.md#7-positions-and-encoding):
line and column are 1-based, columns count Unicode scalar values, byte offsets
are 0-based, and ends are exclusive. Invalid UTF-8 is retained as bytes,
flagged, and decoded for text positions with the fixture's explicit replacement
mapping. CRLF counts as one line ending while offsets remain offsets into the
original bytes. A parser's native point type is therefore an input to the
adapter, never the public range representation.

The minimum range fixture matrix is:

| ID | Input shape | Required assertion |
|---|---|---|
| `R-ASCII` | one ASCII line and an empty line | byte offset equals character count; line starts are 1-based |
| `R-UTF8` | 2-, 3-, and 4-byte UTF-8 scalars before and inside a capture | columns count scalars, offsets count bytes |
| `R-COMBINE` | precomposed scalar beside a combining-mark sequence | each scalar counts one; normalization is not applied |
| `R-TAB` | tabs before and inside a capture | a tab counts one character; display width is not substituted |
| `R-CRLF` | mixed CRLF and LF endings | each ending advances one line; CRLF occupies two bytes |
| `R-INVALID` | invalid bytes between valid UTF-8 spans | bytes are preserved, the file is flagged, and replacement mapping is stable |
| `R-LONG` | a line beyond the normal line-size target | lookup remains bounded and returns exact exclusive offsets |
| `R-BOUNDARY` | capture starts/ends at line and multi-byte boundaries | start/end are not widened or rounded |
| `R-EMPTY` | zero-byte file and zero-width capture | line/column and optional offset follow the documented empty-file convention |

Every row has a hand-reviewed expected byte range, line/column range, decoded
text, and file flag set. The matrix is reused by F1, F4 cancellation, the
range-conversion fuzz target, and the conformance snapshots.

For `R-INVALID`, the fixture's canonical text view maps each maximal invalid
UTF-8 subsequence to one `U+FFFD`; valid scalars on either side retain their
original byte spans. An adapter may use a parser's decoded view internally,
but it must normalize to this mapping before publishing positions.

Parser timeout handling is capability-probed at the pinned binding. When the
binding exposes a native timeout, progress callback, or cancellation flag, the
spike measures that mechanism at the F4 safe point. Query/fact construction
uses the same cooperative signal. If a parser call cannot be interrupted
safely, it runs in an isolated worker boundary whose termination is measured;
the parent receives no `FactBatch` from a terminated worker. Every timeout or
cancellation yields a bounded skip/diagnostic and leaves the authoritative
revision unchanged. An implementation never force-kills a thread that still
owns parser or store state.

F-004 compares three byte-to-character line-index shapes before choosing an
implementation: (A) line starts plus a complete per-byte scalar map, (B) line
starts plus scalar checkpoints every fixed byte stride, and (C) line starts
with bounded decode from the nearest line start. The benchmark varies line
length, non-ASCII density, lookup locality, and invalid-byte density, and
reports construction bytes, lookup latency, memory, and cancellation checks.
The public contract stays independent of the chosen representation.

### F-004 initial prototype result — 2026-08-18

The standalone report is [`Experiment Result: F-004`](results/f004-line-index.md);
the summary below remains here so the preparation protocol and its measured
outcome stay adjacent.

The disposable std-only prototype is retained at
[`f004_line_index.rs`](f004_line_index.rs). It validates all three shapes
against the same scalar-boundary oracle, then measures optimized construction
and 2,048 lookups repeated three times for sequential, deterministic-random,
and hot (32-boundary) locality. The command and environment were:

```sh
rustc --edition=2024 -O docs/experiments/f004_line_index.rs -o /tmp/repin-f004
/tmp/repin-f004
```

| Field | Value |
|---|---|
| Platform | Linux 7.1.4-204.fc44.x86_64, AMD Ryzen 7 5825U, 16 logical CPUs |
| Toolchain | `rustc 1.97.1`, `cargo 1.97.1` |
| Workloads | 256 KiB target; 80-byte and 4,096-byte lines; ASCII, UTF-8, and invalid-byte fixtures |
| Lookup unit | scalar-boundary byte offset; line/column result; 6,144 lookups per reported timing |
| Correctness | all shapes returned identical line/column pairs for every generated boundary |

Representative output (microseconds; one optimized run) shows the trade-off:

| Workload/metric | Full map (A) | Checkpoint, 64-byte stride (B) | Line scan (C) |
|---|---:|---:|---:|
| UTF-8, 4,096-byte lines: build | 992 | 586 | 153 |
| UTF-8, 4,096-byte lines: random lookup | 204 | 1,020 | 16,346 |
| UTF-8, 4,096-byte lines: hot lookup | 50 | 319 | 394 |
| 80-byte lines: estimated index bytes | 1,074,484 | 129,480 | 25,896 |
| 4,096-byte lines: estimated index bytes | 1,049,100 | 65,752 | 520 |

The full map is the fastest and most predictable lookup path but costs roughly
four bytes per input byte. The line scan is smallest but degrades with long
lines and random access. Checkpoints bound scan work while using about one
eighth of the full-map memory in these fixtures. Invalid-byte density changed
the absolute timings but not the ordering, and the three shapes remained
equivalent under the prototype's canonical invalid-run mapping.

This closes the F-004 prototype/measurement task, not the final representation
decision. Shape B is the provisional follow-up candidate because it makes the
scan bound explicit without committing a per-byte allocation; a larger corpus
and cancellation instrumentation remain Linux PoC evidence before plan
finalization. Platform-specific reruns are post-PoC work.

## 3. F2 — filesystem discovery and containment

Evaluate `cap-std`, `ignore`, and `globset` as mechanisms, not as the safety policy. Compare canonicalize-then-open with opening relative to pre-opened root capabilities, especially under symlink swaps.

### F2 Cases

1. Walk nested ignore files, negation rules, hidden files, dependency/build directories, and engine-specific exclusions.
2. Exercise multiple roots, root relocation, non-UTF-8 path components where supported, and case-sensitive/case-insensitive filesystems.
3. Attack traversal, absolute paths, symlink escapes, symlink cycles, and symlink swaps between validation and read.
4. Reopen discovered paths through root directory capabilities; verify a renamed/replaced component cannot redirect the read outside the root.
5. Sniff binaries with misleading extensions and text files without extensions.
6. Overflow directory depth/count limits and verify bounded skips plus coverage.
7. Compare a discovered snapshot with a second reconciliation scan after concurrent filesystem mutations.

### F2 Pass conditions

- Selection matches documented precedence and never widens through query scope.
- Every returned path is normalized and root-relative.
- Escape and indeterminate containment fail closed.
- Capability-relative opens or an equally strong mechanism prevent check/open races from causing an out-of-root read.
- Cycles terminate and mutations cannot cause an out-of-root read.
- Every omission has a queryable reason.

### F2 preparation: adversarial path manifest and open protocol

The path fixture manifest uses stable IDs and expected outcomes rather than
platform-specific error strings:

| ID | Attack or condition | Expected outcome |
|---|---|---|
| `P-TRAVERSAL` | `../`, repeated separators, and encoded traversal | reject before open |
| `P-ABSOLUTE` | absolute path and drive/UNC-shaped path | reject unless it is the configured root itself |
| `P-ESCAPE` | symlink from an in-root entry to an out-of-root target | reject and record containment reason |
| `P-CYCLE` | two or more symlinks forming a cycle | terminate with bounded skip |
| `P-SWAP` | replace a checked directory component before read | fail closed; no out-of-root bytes returned |
| `P-DEEP` | path depth and component length over configured limits | bounded skip with limit reason |
| `P-ENCODING` | non-UTF-8 path components where the platform permits them | preserve opaque path identity or skip explicitly; never lossy-collide |
| `P-CASE` | case-only aliases on case-sensitive and case-insensitive filesystems | report the platform observation; do not merge distinct paths by guesswork |
| `P-MUTATE` | create/delete/rename during a reconciliation walk | converge after the next scan or report incomplete coverage |

The root-capability spike follows one read protocol:

1. Activate a named root and retain its directory capability plus a stable
   `RootId`.
2. Have the walker emit only a normalized root-relative path and its observed
   metadata; never reopen an absolute path returned by the walker.
3. Reopen the path relative to the root capability, with no-follow behavior for
   the final component where the platform exposes it.
4. Read bytes from the opened handle, then re-stat the same handle. A mismatch
   between the pre-read and post-read identity/size metadata yields an unstable
   snapshot; it is retried only under the bounded F3 policy and otherwise fails
   closed.
5. Verify the resulting snapshot's root, relative path, byte length, and hash
   before handing it to extraction.

The experiment compares this protocol with canonicalize-then-open. The latter
is a comparison baseline only; it cannot pass the `P-SWAP` case by relying on
path checks performed before a separate open.

Content sniffing is a separate decision from path selection. The comparison
starts with two strategies over a bounded prefix (default 8 KiB):

| Strategy | Signal | Known risk to measure |
|---|---|---|
| `S0-inhouse` | NUL/control-byte policy plus UTF-8 decode status; no extension lookup | false positives on generated/binary-looking source and false negatives on text encodings |
| `S1-maintained` | a maintained content-inspection crate, pinned and configured with an explicit byte budget | dependency/build surface and a policy that may disagree with Repin's exclusion rules |

The fixture includes source with embedded NUL-like escapes, UTF-16/UTF-32
without a BOM, compressed/archive bytes, minified text, generated files, and
text with invalid UTF-8. The selected policy must be deterministic, bounded,
and fail closed when it cannot classify safely. A classifier result is a
selection input and a diagnostic; it never overrides an explicit secret or
engine-directory exclusion. F2 records precision/recall against the reviewed
fixture labels before F-009 is closed.

## 4. F3 — hash and update preparation protocol

Evaluate BLAKE3 and the prepare/revalidate/commit flow from [`docs/incremental.md` §5](../incremental.md#5-transactions).

### F3 Cases

1. Hash representative file-size distributions and compare hashing cost with parsing cost.
2. Prepare extraction, mutate the file before commit, and verify revalidation rejects stale facts.
3. Deduplicate host, watcher, scan, and VCS reports with identical bytes.
4. Exercise create/delete/recreate and rename/coalescing sequences.
5. Verify content hashes never affect node IDs.

### F3 Pass conditions

- Identical bytes deduplicate regardless of event origin.
- Changed bytes cannot commit facts prepared from an older snapshot.
- Revalidation does not require holding the store writer during parsing.
- Hash representation is versioned/algorithm-tagged where persisted or exposed as evidence.

### F3 preparation: snapshot and revalidation state machine

Every prepared file is represented by a spike-local `InputSnapshot`:

```text
InputSnapshot
  rootId:          RootId
  relativePath:    Path
  source:          host_supplied | filesystem
  bytes:           Bytes
  hash:            Hash
  observedMeta?:   FileIdentityAndSize
  observedAt:      Timestamp
```

The prepare/revalidate state machine is:

```text
UNSEEN
  → PREPARING       read supplied bytes or open/read through root capability
  → PREPARED        hash, parse, extract, and record base graph revision
  → REVALIDATING    verify path selection, current file identity, and hash
  → READY_TO_COMMIT authoritative store transaction may begin
  → COMMITTED       revision acknowledged
```

Any failed check transitions to `STALE`, never to `READY_TO_COMMIT`:

```text
PREPARING/REVALIDATING → STALE → REPREPARE (at most twice per API call)
                                      └→ CONFLICT if still stale
```

For `filesystem` input, revalidation reopens the selected path and compares
the current tagged hash and file identity/size with the prepared snapshot. For
`host_supplied` input, the supplied bytes and tagged hash are authoritative for
that call, while selection/root checks are still repeated at commit; if the
host also supplies a file identity, it is compared as an additional guard.
The engine never commits a fact batch whose bytes cannot be tied to the
selected root/path and the call's input identity.

The state machine is deliberately outside the writer transaction. A stale
plan retains/coalesces reconciliation work and returns the explicit conflict
outcome from [Incremental Updates §5](../incremental.md#5-transactions) after
the retry budget is exhausted.

### F3 preparation: hash/read benchmark matrix

The spike separates storage read cost from hashing cost by running each size
band with (a) bytes already resident in memory, (b) a fresh file read with hash
disabled, and (c) the same read with the candidate hash enabled. The matrix
uses representative files at 0 B, 1 KiB, 4 KiB, 64 KiB, 1 MiB, 16 MiB, and
the upper bound of each normal corpus band, plus one bounded pathological long
file. It records wall time distributions, bytes/s, CPU time where available,
and peak memory; it does not turn any observed throughput into a product
promise. Hash equality tests use at least two algorithms or an intentionally
different digest label to prove that equal digest bytes with different
algorithm tags are not equal.

## 5. F4 — cancellation and concurrency model

Compare three disposable Linux x86_64/glibc execution models with bounded
admission: a synchronous core with blocking adapters, a synchronous core with
async only at service/remote adapter boundaries, and Tokio async orchestration
with bounded blocking work. The separate `repin-f4-spike` binary preserves the
existing runner as the package default and keeps Tokio out of the synchronous
build. This is the fully featured PoC; Tier 2 and lower-tier/platform work is
not implemented until this PoC is complete.

### F4 Cases

1. Cancel crawl, 1 MiB-chunked read/hash, tree-sitter parse/query callbacks,
   1,000-edge resolution batches, 64 KiB regex chunks, context assembly, store
   preparation, and benchmark loops.
2. Apply relative timeouts and absolute deadlines; verify the earlier bound wins.
3. Cancel before the authoritative commit, during a non-cancellable store commit, and during derived-index reconciliation.
4. Saturate concurrent reads and update requests; verify bounded queues,
   update coalescing, overflow escalation to a root rescan, and recorded queue
   and worker maxima.
5. Start and stop a watch coordinator 100 times with pending synthetic events,
   including repeated idempotent shutdown.
6. Exercise bounded loopback service and remote-model protocols under the same
   concurrency limits.
7. Terminate a deliberately non-cooperative child that receives only input
   bytes; it must return neither parser state nor a fact batch.

The core worker count is `min(4, available_parallelism)` and ingress capacity
is eight requests per worker. Sync and hybrid use a bounded `sync_channel`;
async uses bounded Tokio `mpsc` plus a semaphore before `spawn_blocking`. The
hybrid adapter owns a two-thread Tokio I/O runtime; Tokio types do not enter
the simulated core contract.

### F4 Pass conditions

- Cancellation latency has a measured upper bound at defined safe points.
- No cancellation exposes a partial authoritative revision.
- Shutdown is prompt and idempotent.
- Queue and worker counts remain bounded under saturation.
- The selected runtime model has a documented reason and does not leak into portable API semantics.

The fixed evidence command is:

```text
cargo run --release --locked --offline --features async-runtime \
  --bin repin-f4-spike -- run --model all --profile full \
  --output docs/experiments/results/raw/f4-tier1-20260818
```

`smoke` is the reduced Linux PoC profile for local verification. The full
profile uses fixture seed `repin-f4-1`, one warmup, 30 cancellation samples,
and five throughput samples per model/workload. Raw JSON is retained with
p50/p95/maximum, throughput, queue depth, worker/thread counts, binary size,
and clean-build timing. Set `REPINF4_CLEAN_BUILD_MS` when invoking the runner
after measuring a clean release build so that timing is retained in the
manifest and report.

### F4 preparation: proposed safe-point budgets and workloads

These are measurement targets for the spike, not user-visible guarantees. A
long-running operation MUST check cancellation at the earlier of the listed
unit bound or the operation deadline:

| Operation | Safe-point target |
|---|---|
| directory enumeration | every entry batch of at most 256 entries or 25 ms |
| file read/hash | every 1 MiB or 25 ms |
| parser/query/fact construction | every 25 ms where the binding exposes a check; otherwise the isolated-worker boundary is measured |
| resolution and graph traversal | every 1,000 facts/edges or 25 ms |
| direct regex scan | every 64 KiB or 25 ms |
| context assembly | every candidate or 25 ms |
| worker shutdown | cancellation acknowledged within 250 ms after the current non-cancellable commit section |

The 25 ms and 250 ms values are proposed workstation-interaction targets;
spikes report observed maxima and tail distributions under Micro and Large
normal workloads before they become API promises. A deadline always wins over
these targets. Native store/index commits remain bounded atomic sections: a
cancel request stops the next section and cannot expose a partial authoritative
revision.

Runtime comparison workloads are fixed as follows:

| Workload | Synchronous-core question | Async-runtime question |
|---|---|---|
| local crawl/index | Can bounded worker threads saturate useful I/O/CPU without unbounded queues? | Does async reduce resource use or improve cancellation materially? |
| watch session | Can a coordinator normalize bursts and shut down idempotently? | Does async simplify event ownership without leaking into the core? |
| service adapter | Can blocking calls be isolated behind bounded workers? | Does connection multiplexing require async types at the adapter boundary only? |
| remote model port | Can optional provider work be isolated and cancelled? | Does network concurrency justify an async adapter while deterministic paths stay sync? |

The default hypothesis remains a synchronous core with explicit cancellation,
bounded worker pools, and async only at an adapter boundary if a measured
workload requires it. The decision records throughput, cancellation tails,
queue bounds, shutdown behavior, dependency/build cost, and API leakage.

## F8: Runtime daemon and project contexts

This experiment validates the process and filesystem contract in [Runtime and
IPC](../runtime.md) before the runtime is treated as an implementation detail.
It targets Linux x86_64/glibc with pathname Unix-domain sockets and a virtual
clock for idle-eviction tests. It is a runtime experiment, not an endorsement
of remote transport or a second deployment topology.

### F8 cases

1. **Cold-start race.** Launch a bounded set of clients concurrently with no
   socket or lease. Verify exactly one same-binary daemon candidate acquires
   the per-user singleton lease, publishes one socket, and serves all clients;
   losing candidates exit and their clients reconnect. Repeat with a stale
   socket, a live socket, a malformed readiness response, and a candidate that
   dies before readiness.
2. **Discovery.** Build nested repositories with initialized and incomplete
   `.repin/graph.redb` markers. Verify nearest-ancestor selection, continued
   traversal past `.repin` without a database or a database without its
   directory, `PROJECT_NOT_INITIALIZED` at the top, and explicit `AtRoot`
   override. Include a symlinked parent directory and verify canonical parent
   resolution selects the physical ancestor.
3. **Initialization.** Run `repin init` against an absent state directory,
   an existing `.repin` without a database, and an existing database. Verify
   private creation, lock acquisition, database creation, and no overwrite of
   existing bytes. Interrupt creation at each publication point and verify
   restart leaves either a recoverable incomplete marker or a valid project,
   never an apparently initialized partial database.
4. **Context registry.** Connect two clients to one canonical database and
   verify one context, one watcher, one writer handle, and one shared revision.
   Copy the database to another canonical path and verify the copied project
   receives a distinct context, lock, revision stream, and pending-work queue.
5. **Alias guard.** While one context is active, address its database through
   a symlink, renamed path, bind mount, hard link, or alternate spelling where
   supported. Verify the second open returns `PROJECT_STATE_ALIAS`, never
   creates a second context, and never writes through the alias. Rename,
   replace, or delete the active state and verify the original context fails
   closed.
6. **State degradation.** Attach to invalid/corrupt and newer graph databases
   with a safe working tree. Verify direct retrieval remains bounded and
   available, graph access reports the precise state error, and no automatic
   overwrite or rebuild occurs. Hold the project writer lock in an external
   process and verify observer attachment, direct retrieval, and
   `PROJECT_LEASE_UNAVAILABLE` for graph writes.
7. **Idle lifecycle.** Use a virtual clock to detach the final client and
   advance exactly `600,000 ms`. Verify watcher registration alone does not
   keep the context active, in-flight work and mandatory recovery defer
   eviction, an active sibling context is unaffected, and the context closes
   stores/indexes before releasing its project lock. After the final context
   unloads, verify the daemon closes its socket before releasing the singleton
   lease.
8. **Crash and client failure.** Kill the daemon during reads, a watcher
   update, and an authoritative commit. Verify OS handle release, no second
   writer, stale rendezvous repair, and independent project recovery on
   restart. Terminate one client and verify other clients, contexts, and
   requests continue unaffected.
9. **Protocol and bounds.** Negotiate supported and unsupported versions;
   verify `PROTOCOL_MISMATCH`, bounded frames/admission, request IDs,
   progress, deadlines, cancellation, and disconnect behavior across a
   project-bound connection. A request must not smuggle a second project path
   after the handshake.

### F8 artifacts and pass conditions

Retain the process-start manifest, daemon candidate outcomes, socket/lease
transcript, canonical and physical path observations, context registry trace,
revision observations, lock ownership trace, virtual-clock timeline, protocol
frames with content redacted, and recovery transcript. Record the Linux kernel,
filesystem, toolchain, binary hash, fixture paths, and exact test seed.

The experiment passes only if one daemon is elected in every cold-start race;
nearest and explicit selection follow the contract; aliases cannot duplicate
an active context; copied databases remain isolated; degraded attachment never
loses safe direct retrieval; idle eviction and final daemon shutdown release
resources in the required order; daemon death releases all project locks; and
all runtime errors and protocol bounds are observable without leaking secrets.

### F8 Linux PoC result — 2026-08-19

The disposable `repin-f8-spike` harness exercises the Linux pathname
Unix-domain-socket contract with the same binary acting as daemon candidates.
The retained [F8 report](results/raw/runtime-review-20260819/f8-runtime/F8-report.json)
passes all 14 cases: singleton cold-start election, live/malformed/stale
startup repair, initialization, discovery, canonical context sharing and
copied-path isolation, active filesystem aliases, degraded and observer
attachment, bounded protocol behavior, client detachment, crash restart,
virtual-clock idle eviction, and final daemon exit. This closes the F8
experiment task only; the harness is not the production daemon implementation.

## 6. F5 — watch adapter (`notify`, deferred until I3)

### F5 Cases

1. Capture native events for create, modify, delete, rename, editor atomic-save, branch switch, and burst writes on each target platform.
2. Force backend overflow or simulate loss and verify escalation to reconciliation scan.
3. Vary debounce duration and event ordering; compare final committed graph state.
4. Combine host notification and watch events for identical content.
5. Replace watched directories and exercise symlink behavior.

### F5 Pass conditions

- Normalized output converges independently of raw ordering and debounce.
- Duplicate reports result in one content update.
- Loss/overflow is observable and escalates; it never produces silent partial state.
- Watch absence or failure leaves explicit notification and scanning complete.

## 7. F6 — direct regex and VCS adapters

### F6 preparation: comparison contracts

The direct-regex comparison uses a deliberately bounded baseline syntax:

```text
literal · character class · Unicode properties · grouping · alternation
quantifiers (*, +, ?, {m,n}) · ^ and $ anchors · multiline mode
```

Backreferences, look-around, recursion, replacement expressions, and
engine-specific code execution are outside this baseline. A candidate may
support more syntax internally, but the advertised contract cannot depend on
it until its compile/memory/cancellation behavior is measured. Unsupported
syntax returns `INVALID_QUERY` with the supported feature set; it does not
silently reinterpret the pattern. Match spans are reported in original byte
offsets plus the normalized line/Unicode-scalar positions from F1.

The VCS comparison records one normalized result shape for both candidates:
current revision/branch identity, changed paths, change kind, and a reason for
falling back to a scan. Git subprocess cases run without a shell, with a
sanitized environment, bounded stdout/stderr, an explicit executable path
policy, and cancellation that kills the child and reaps it. Hooks, repository
configuration, aliases, external diff drivers, and user-provided executable
selection are disabled unless a case explicitly tests the configured policy.

### F6 Cases

1. Compare `regex` and `regex-automata` for literal-heavy, Unicode, multiline, anchored, alternation-heavy, and intentionally expensive supported patterns.
2. Define rejected syntax and verify errors include the supported set/limits.
3. Measure compile memory/time and scan cancellation latency across corpus size bands.
4. Compare `gix` with a bounded Git subprocess for changed sets, dirty files, branch switches, shallow clones, submodules/worktrees, ignored files, and rewritten/unreachable revisions.
5. For subprocess operation, sanitize environment, bound stdout/stderr, cancel promptly, avoid shell invocation, and test missing/incompatible Git.

### F6 Pass conditions

- Direct regex search has documented syntax, exact spans, compile/memory limits, and bounded cancellation behavior.
- Unsupported regex features fail explicitly rather than changing meaning.
- The VCS candidate returns correct normalized changes for the supported matrix and cleanly falls back to a scan.
- Repository content cannot alter executable selection, arguments, configuration, hooks, or output bounds.

The experiment must compare the same fixture and changed-set oracle through
`gix` and the bounded subprocess adapter. A missing Git executable, shallow or
rewritten history, submodule/worktree, dirty tree, and branch switch each have
an explicit expected fallback or normalized result; “could not determine” is
not treated as “no changes”.

## 8. F7 — test, fuzz, benchmark, and dependency toolchain

Evaluate `proptest`, `cargo-fuzz`, Criterion, `iai-callgrind`, `cargo-deny`,
`cargo-audit`, `cargo-sbom`, and `cargo-auditable` with minimal representative
targets.

### F7 Cases

1. Generate seeded update sequences and shrink a deliberately injected convergence defect.
2. Snapshot a normalized graph with `insta` and exercise human-reviewed update flow; test CLI JSON/exit behavior with `assert_cmd`.
3. Fuzz range conversion, path normalization, redaction, query parsing, and one parser adapter.
4. Compare Criterion variance with instruction-count measurements for identity, ranking, and edge-key encoding.
5. Configure license/source/advisory checks and generate a dependency inventory/SBOM candidate.
6. Test how native grammar and vector dependencies appear in release artifacts and policy reports.

### F7 Pass conditions

- Property failures shrink to reproducible fixtures and retain their seeds.
- Fuzz crashes become permanent regression inputs.
- Benchmark tools detect an injected regression without unstable thresholds.
- Dependency policy fails on an intentionally disallowed license/source/advisory.
- The selected release process can inventory Rust and native components.

### F7 preparation: reusable quality artifacts

The first spike repository should contain these reviewable artifacts before
candidate measurements begin:

| Artifact | Required contents |
|---|---|
| property strategy | seeded valid `FileChange` sequences, generated final filesystem state, and a graph-equality oracle |
| fuzz manifest | range conversion, path normalization, redaction, query parsing, and one parser adapter target |
| fuzz resource policy | maximum input bytes, execution time, corpus bytes, retained crash count, and sanitizer/toolchain configuration |
| benchmark map | Criterion for noisy end-to-end distributions, iai-callgrind for stable instruction-count paths, and fixed-corpus harnesses for wall-clock scaling |
| dependency policy probe | intentionally disallowed license/source/advisory fixtures plus a report showing Rust and native components |
| CI matrix | per-change fast checks, full Linux PoC conformance/convergence, and scheduled fuzz/benchmark jobs; platform-expansion jobs are post-PoC |

These are experiment inputs. They do not yet select a snapshot library,
fuzzer, SBOM format, or release policy; those choices remain candidate
decisions until the probe runs.

### F7 follow-up evidence: Q-003, Q-006, Q-007, Q-008, and Q-012

The disposable spike now contains the reviewed Q-series follow-up artifacts:

| Q case | Retained evidence | Provisional observation |
|---|---|---|
| Q-003 | `assert_cmd` 2.2.2, `insta` 1.48.0, normalized snapshot, and CLI integration tests | snapshot review and stdout/stderr/exit-code checks pass |
| Q-006 | [`deny.toml`](foundation_spike/deny.toml), GPL fixture, and generated local Git-source fixture | the clean graph passes with duplicate-version warnings; disallowed license/source fixtures fail closed |
| Q-007 | [advisory response policy](advisory-policy.md), empty exception file, evaluator tests, and `time` 0.1.45 fixture | vulnerability/unsound findings block; unmaintained/notice findings warn; exact exceptions expire within 30 days |
| Q-008 | SPDX JSON 2.3, CycloneDX JSON 1.6, metadata comparison, isolated USearch inventory, and auditable binary reports | tree-sitter grammar packages and USearch/CXX owning crates are inventoried; raw native C/C++ is not a separate Cargo component |
| Q-012 | [raw pinned-tool run](results/raw/q-release-tools-20260818/report.json) and its manifest/artifacts | all 17 positive/negative cases pass on the Linux PoC host; the recommendation remains deferred |

The Q runner records exact tool pins, tool binary hashes, the Rust/Cargo
versions, lockfile/source/artifact hashes, advisory-database commit/date,
commands, exit codes, normalized findings, generated SBOMs, and binary
inspection reports. Its manifest and report expose the Q case IDs so a later
run can be compared without relying on prose. SPDX JSON 2.3 is the provisional
canonical release SBOM; CycloneDX JSON 1.6 is retained as a compatibility
comparison. USearch is present only in the isolated Q-008 inventory fixture;
no S3 vector behavior or production dependency was added.

## 9. Outputs

Each experiment concludes with `accept | reject | defer | revise experiment`,
evidence links, known limits, and implications for the implementation
milestones. The per-family ledger is [Experiment Results](results/index.md);
pending and deferred reports are explicit rather than implied acceptance.
Plan finalization turns accepted outcomes into ADRs and pins versions/MSRV/
support policy; experiments themselves do not make final decisions.
