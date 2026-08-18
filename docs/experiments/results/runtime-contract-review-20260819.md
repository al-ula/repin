# 2026-08-19 Linux PoC Experiment Review

## Status

This is a fresh, provisional rerun of the executable Linux foundation
experiments after the global-daemon and path-addressed project-context design
was documented. It records evidence only; all implementation decisions remain
deferred.

The retained raw outputs are the [default foundation batch](raw/runtime-review-20260819/foundation-default/batch.json),
the [feature-enabled foundation batch](raw/runtime-review-20260819/foundation-features/batch.json),
the [foundation follow-up batch](raw/runtime-review-20260819/foundation-followup/batch.json),
the [F4 smoke report](raw/runtime-review-20260819/f4-smoke/F4-report.json), and
the [F8 runtime report](raw/runtime-review-20260819/f8-runtime/F8-report.json),
the [original Q release-tool report](raw/runtime-review-20260819/q-release-tools/report.json),
and the [Q-014 rerun](raw/runtime-review-20260819/q-release-tools-q014/report.json).

## What was rerun

The disposable [foundation spike](../foundation_spike/README.md) was run on
Linux x86_64/glibc with Rust `1.97.1` and Cargo `1.97.1`:

```text
cargo fmt -- --check
cargo check --locked --offline
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The default F1/F2/F3/F6/F7 batch and the feature-enabled batch
(`gix-adapter,sniff-adapter`) both completed without a hard blocker. The
feature-enabled batch closed the optional `infer` and `gix` coverage gaps:

| Run | Result |
|---|---|
| Default F1/F2/F3/F6/F7 | F1 13/13, F2 8/9, F3 5/5, F6 15/16, F7 5/9; overall inconclusive |
| Feature-enabled F1/F2/F3/F6/F7 | F1 13/13, F2 9/9, F3 5/5, F6 16/16, F7 5/9; overall inconclusive |
| F-017/F-018/F-009/F-019/F-014/F-015/F-020 | 62/62; complete, decision deferred |
| F4 smoke, sync/hybrid/async | 72/72; complete, overall inconclusive |
| F8 daemon/context runtime | 14/14; complete, pass, decision deferred |
| Q-014 release-tool rerun with writable advisory state | 17/17; complete, overall inconclusive |

The F4 smoke run required permission to bind a loopback listener. It exercised
the local spike service, not the planned user-wide daemon or its central Unix
socket. Its selection result remains “sync core remains default”; the hybrid
service p95 benefit was about 3.3%, below the 25% selection threshold.

The first Q rerun had four environment failures because the audit process could
not fetch or lock its advisory database under the original Cargo home. Q-014
was then rerun with a writable copied advisory database and `--no-fetch`; all
17 pinned-tool, policy, advisory, SBOM, build, and binary-inspection cases
passed. The overall release decision remains deferred as required by the
experiment contract.

## Runtime-contract review

The documentation change adds the F8 contract. The disposable spike now has an
F8 harness that starts the same binary as daemon candidates and exercises the
contract on Linux x86_64/glibc. All 14 F8 cases passed: cold-start election,
live/malformed/stale startup repair, initialization, discovery, canonical
context sharing and copied-path isolation, active hard-link/symlink/replaced
path guards, degraded and observer attachment, bounded protocol behavior,
client detachment, crash restart, virtual-clock idle eviction, and final
daemon exit. F8 is therefore complete as an experiment; it is not a production
daemon implementation.

The following existing evidence remains valid and does not need to be
invalidated by the topology change:

- F1/F2/F3/F6/F7 and their follow-ups test extraction, discovery, update,
  adapters, and quality artifacts independently of deployment topology.
- F4 supports cancellation/concurrency contract decisions for the local
  service core, but is not evidence for global-daemon startup, connection
  binding, or project-context isolation.
- S1, S2, and S4 remain pending as recorded; S3 remains intentionally deferred.

The F8 harness uses a virtual clock for the normative `600,000 ms` idle
threshold and an experiment-only admin channel for snapshots/clock advancement;
neither changes the public runtime contract. Full publication-point interruption
and non-Linux/bind-mount expansion remain post-PoC work.

## Disposition

No prior experiment needs a full rerun solely because the daemon is now global.
The two follow-ups from the initial review are now closed:

1. F-024 has a retained 14/14 F8 run. The production runtime still requires a
   later implementation/conformance pass.
2. Q-014 has a retained 17/17 rerun using writable advisory state.
3. Keep the current F1/F2/F3/F6/F7 and follow-up evidence as retained,
   provisional evidence; do not convert any “inconclusive” result into a
   selection from this rerun.

The foundation runner’s embedded run identifier still says
`foundation-tier1-20260818`; the artifact directory and this review are dated
2026-08-19. The date distinction is intentional so the rerun is not confused
with the earlier retained ledger entries.

## Reproduction commands

From `docs/experiments/foundation_spike`:

```text
cargo run --release --locked --offline -- run-all --output /tmp/repin-foundation-rerun-20260819
cargo run --release --locked --offline --features gix-adapter,sniff-adapter -- run-all --output /tmp/repin-foundation-feature-rerun-20260819
cargo run --release --locked --offline --features gix-adapter,sniff-adapter --bin repin-foundation-followup -- run-all --output /tmp/repin-foundation-followup-rerun-20260819
cargo run --release --locked --offline --features async-runtime --bin repin-f4-spike -- run --model all --profile smoke --output /tmp/repin-f4-smoke-20260819
cargo run --release --locked --offline --bin repin-f8-spike -- run --output /tmp/repin-f8-runtime-20260819-v2
env CARGO_HOME=<writable-cargo-home> python3 scripts/run_q_release_tools.py --tool-root /tmp/repin-q-tools --no-fetch --output /tmp/repin-q-release-tools-q014-20260819-v2
```
