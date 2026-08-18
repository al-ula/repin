# Experiment Results

This directory is the result ledger for the disposable experiment families in
[`Rust Foundation Experiments`](../rust-foundation.md) and
[`Storage Adapter Experiments`](../storage.md). A report records what was run,
what was observed, and the provisional recommendation it may inform later; it
never turns a candidate into an accepted dependency by implication.

After an evidence run, actual decision-making remains deferred. The exception
is a demonstrated violation of a normative contract that prevents dependent
work; that is a hard blocker. Incomplete coverage, unavailable optional tools,
and an inconclusive comparison are recorded as follow-up evidence, not hard
blockers.

`pending` means the experiment is specified but has no retained run result.
`deferred` is an intentional planning outcome, not a missing report.
`recorded (Tier 1; inconclusive)` means that evidence was retained without
accepting, rejecting, or selecting an implementation default.

| Experiment | Status | Result | Provisional recommendation (decision deferred) |
|---|---|---|---|
| [F1 — tree-sitter extraction](f1-tree-sitter.md) | recorded (Linux PoC; inconclusive) | deterministic capture/build observations retained | retain parser/grammar candidates provisionally; finish Linux oracle and cancellation evidence |
| [F2 — filesystem discovery](f2-filesystem.md) | recorded (Linux PoC; inconclusive) | selection, capability, and sniff observations retained | retain protocols and alternatives provisionally; finish Linux adversarial, reconciliation, encoding, and limit cases |
| [F-008 — root-capability opens](f008-root-capability.md) | complete on Linux PoC | capability path rejected all 200 race attempts | retain the protocol provisionally; platform expansion is post-PoC |
| [F3 — hash/update protocol](f3-hash-update.md) | recorded (Tier 1; inconclusive) | hash and stale-snapshot observations retained | retain BLAKE3 and snapshot policy provisionally; finish read, reprepare, and coalescing evidence |
| [F4 — cancellation/concurrency](f4-cancellation.md) | recorded (Linux PoC; hybrid-benefit audit unresolved; overall inconclusive) | three serial full replicates pass all 72 behavioral cases; the 12-cell affinity/order/concurrency audit did not reproduce the initial sync slowdown | retain the synchronous core contract; close runtime selection as inconclusive; require a separately approved plan before any new full F4 selection run; defer Tier 2/non-Linux work until the fully featured PoC review |
| [F5 — watch adapter](f5-watch.md) | deferred | deferred until I3 | reopen when watching enters implementation |
| [F6 — regex/VCS adapters](f6-regex-vcs.md) | recorded (Linux PoC; inconclusive) | regex, subprocess, and gix observations retained | retain both adapter approaches provisionally; finish Linux bounds, cancellation, and VCS evidence |
| [F-017 — F1 evidence follow-up](f017-f1-followup.md) | complete | reviewed query packs, deterministic captures, full range oracle, and parser cancellation retained | defer grammar/query production selection; platform expansion remains post-PoC |
| [F-018 — F2 adversarial filesystem follow-up](f018-f2-followup.md) | complete | complete Linux path matrix, omission reasons, reconciliation, and sniff corpus retained | retain capability-relative protocol provisionally; F-009 remains a separate policy evaluation |
| [F-009 — content-sniffing comparison](f009-content-sniff.md) | complete | in-house and `infer` metrics retained; maintained unknowns explicit | retain in-house check provisionally; defer dependency/policy selection |
| [F-019 — F3 hash/update follow-up](f019-f3-followup.md) | complete | all corpus-band distributions, two-retry conflict, graph equality, and coalescing retained | retain protocol evidence; defer representation and thresholds |
| [F-014 — regex comparison](f014-regex.md) | complete | syntax, unsupported constructs, spans, compile/RSS, and cancellation evidence retained | defer adapter selection |
| [F-015 — VCS comparison](f015-vcs.md) | complete | gix/subprocess matrix, sanitized fallback, bounded output, and cancellation retained | defer adapter selection |
| [F-020 — F6 aggregate follow-up](f020-f6-followup.md) | complete | F-014 and F-015 evidence aggregated without production selection | defer final F6 decision |
| [F7 — quality/toolchain](f7-toolchain.md) | recorded (Tier 1 + Q follow-up; inconclusive) | runner, metadata, fuzz smoke, benchmark, pinned policy/advisory/SBOM tools, negative fixtures, SBOMs, and auditable binary inventories retained | retain the candidates provisionally; use SPDX 2.3 as the provisional canonical SBOM and keep the release decision deferred |
| [F8 — runtime daemon and project contexts](../rust-foundation.md#f8-runtime-daemon-and-project-contexts) | complete (Linux PoC; decision deferred) | 14/14 runtime cases pass in the [2026-08-19 review](runtime-contract-review-20260819.md) | retain the harness evidence; production runtime implementation and post-PoC platform expansion remain separate work |
| [F-004 — byte-to-character line index](f004-line-index.md) | complete | three shapes agree; benchmark recorded | retain checkpoints as a provisional internal baseline; decision deferred |
| [S1 — redb store adapter](s1-redb.md) | pending | no run retained | do not accept the store candidate before recovery evidence |
| [S2 — Tantivy lexical adapter](s2-tantivy.md) | pending | no run retained | verify stale-evidence and repair behavior before selection |
| [S3 — vector adapter](s3-vector.md) | deferred | intentionally not run | reopen only at the semantic-retrieval milestone |
| [S4 — revision/recovery protocol](s4-recovery.md) | pending | no run retained | run with vector absent before finalizing cross-index recovery |

## Reporting rule

Each report separates four things:

- the normative contract that cannot be weakened by a candidate;
- the experiment result, including an explicit `not run` or `deferred` result;
- the provisional recommendation and its limits, without making the later
  decision; and
- the evidence still required before plan finalization.

The reusable structure is [`Experiment Result Template`](../template.md).

The dated [2026-08-19 Linux PoC experiment review](runtime-contract-review-20260819.md)
records the post-topology-change rerun, retains its raw manifests, and
distinguishes the passing F8 harness evidence from the still-separate
production runtime implementation.
