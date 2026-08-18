# Foundation spike runner

Disposable Linux x86_64/glibc PoC runners for F1, F2, F3, F4, F6, F7, and F8.
They are evidence code only; no type or adapter from this workspace is part of
the Repin API. Non-Linux, lower-tier, and additional-architecture runs are
post-PoC scope.

## Commands

```sh
cargo fmt -- --check
cargo check --locked --offline
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings

cargo run --release --locked --offline -- \
  run-all --output ../results/raw/foundation-tier1-20260818

cargo run --release --locked --offline \
  --features gix-adapter,sniff-adapter -- \
  run-all --output ../results/raw/foundation-tier1-features-20260818

# F4 cancellation/concurrency comparison; requires the optional Tokio feature
cargo run --release --locked --offline \
  --features async-runtime --bin repin-f4-spike -- \
  run --model all --profile smoke --output /tmp/repin-f4-smoke

cargo run --release --locked --offline \
  --features async-runtime --bin repin-f4-spike -- \
  run --model all --profile full \
  --output ../results/raw/f4-tier1-20260818

# Private child command used by the isolated-worker cancellation case:
repin-f4-spike child-noncooperative

# Open Rust-foundation follow-up tasks, in dependency order:
cargo run --release --locked --offline \
  --features gix-adapter,sniff-adapter \
  --bin repin-foundation-followup -- \
  run-all --output ../results/raw/foundation-followup-20260818-v7

# F8 global daemon and path-addressed project-context runtime experiment.
# Host permission may be required for pathname Unix-domain socket binding.
cargo run --release --locked --offline --bin repin-f8-spike -- \
  run --output ../results/raw/f8-runtime-20260819
```

The two output directories contain the run manifest, per-experiment JSON,
per-experiment report JSON, and batch summary. The feature-enabled run is the
comparable run for the `gix` 0.86.0 and `infer` 0.19.0 candidate probes.

The runner records the working-tree source digest, exact pins, active features,
toolchain, fixture seed, and platform. Its current results are summarized in
the result reports under `../results/`. All outputs are evidence only:
recommendations remain provisional, actual candidate and implementation
decisions are deferred, and no hard blocker was observed in this batch.

The follow-up runner produces separate `F-017`, `F-018`, `F-009`, `F-019`,
`F-014`, `F-015`, and `F-020` JSON reports plus task-scoped artifacts. Its
manifest records the fixed seed, pins, lockfile SHA-256, toolchain, host
environment, active features, and source revision. Repeat runs compare normalized outputs; timings and dynamic Git
commit IDs remain raw measurements. F-016 is intentionally not run until I3
watching planning begins. The retained reports are linked from
[`Experiment Results`](../results/index.md).

## Q-series quality and release-tool follow-up

Q-003 uses exact test-only pins for `assert_cmd` 2.2.2 and `insta` 1.48.0.
Normal snapshot tests disable updates explicitly:

```sh
INSTA_UPDATE=no cargo test --locked --test q003_quality_tools
```

The experiment-only dependency policy is [`deny.toml`](deny.toml). The
advisory policy and exception format are documented in
[`../advisory-policy.md`](../advisory-policy.md), with the machine-checkable
implementation in `scripts/q_policy.py`.

The pinned Q-012 tools are `cargo-deny` 0.20.2, `cargo-audit` 0.22.2,
`cargo-sbom` 0.10.0, and `cargo-auditable` 0.7.5. Install them into a
disposable tool root and run the retained evidence pass as follows:

```sh
scripts/install_q_tools.sh /tmp/repin-q-tools
python3 scripts/run_q_release_tools.py --tool-root /tmp/repin-q-tools

# Q-014 rerun against a writable, already-populated advisory database:
env CARGO_HOME=<writable-cargo-home> \
  python3 scripts/run_q_release_tools.py \
  --tool-root /tmp/repin-q-tools --no-fetch \
  --output ../results/raw/q-release-tools-20260819
```

The raw run records exact commands, exit codes, tool checksums, lockfile and
source digests, advisory-database metadata, generated SBOM hashes, and
auditable binary reports in
`../results/raw/q-release-tools-20260818/`. The Rust runner manifest now
contains `tool_pins` and the Q task IDs; reports also expose their exact
`case_ids`.

`--no-fetch` is explicit for an offline rerun; it makes the pinned
`cargo-audit` use the supplied advisory database without attempting a network
fetch. The F8 runner starts the same binary as detached daemon candidates,
uses `daemon.lock` and `daemon.sock` below a disposable private runtime
directory, and writes one `F8-report.json` plus the manifest. Its idle
lifecycle uses a virtual clock to advance exactly `600,000 ms`; no ten-minute
wall-clock wait is required. The harness validates election, readiness repair,
discovery, initialization, canonical-path context sharing/isolation, active
aliases, degraded and observer attachment, bounded protocol behavior, crash
restart, client detachment, idle eviction, and final daemon exit. It is an
experiment harness, not the production daemon.

The F4 runner records one warmup plus the profile's cancellation and throughput
samples, raw samples with p50/p95/max summaries, queue and worker bounds,
loopback service/remote throughput, binary size, and an optional clean-build
time supplied as `REPINF4_CLEAN_BUILD_MS`. Its synchronous build does not
compile Tokio; `--features async-runtime` enables the hybrid and async models.

## Private hybrid-benefit audit

The audit is Linux PoC evidence only. It does not select a runtime or change
the synchronous core default. The private diagnostic command runs one matrix
cell (two warmups plus ten measured samples for each service and remote
workload), and writes `probe.json` plus the host sidecar:

```sh
cargo build --release --locked --offline --features async-runtime --bin repin-f4-spike
taskset --cpu-list 0-3 target/release/repin-f4-spike diagnose-hybrid \
  --condition pinned --client-mode matched --order 0 \
  --output /tmp/f4-hybrid-diagnostic-cell
```

`condition` is `pinned` or `unpinned`; `client-mode` is `native` or
`matched`; and `order` selects one of the three fixed model rotations. The
diagnostic JSON schema is
`schemas/f4-hybrid-diagnostic.schema.json`; the aggregate schema is
`schemas/f4-hybrid-audit.schema.json`. `client_max_queue` records the
bounded submitted-but-not-started handoff backlog; its validation bound is the
channel capacity plus the configured client workers, while server queue depth
is measured directly against capacity.

The complete 12-cell matrix is driven serially into
`../results/raw/f4-hybrid-audit-20260818`:

```sh
python3 scripts/run_f4_hybrid_audit.py \
  --binary target/release/repin-f4-spike \
  --root ../results/raw/f4-hybrid-audit-20260818 \
  --clean-build-ms <measured-ms>

python3 scripts/audit_f4_hybrid.py \
  --tier1 ../results/raw/f4-tier1-20260818 \
  --confirmatory ../results/raw/f4-confirmatory-20260818 \
  --probes ../results/raw/f4-hybrid-audit-20260818 \
  --output ../results/raw/f4-hybrid-audit-20260818/aggregate.json
```

The audit compares the original Tier-1 and three confirmatory reports with
the controlled probes, classifies affinity, ordering/warmup, or
client-concurrency effects when the decision rules are met, and otherwise
leaves the runtime selection inconclusive with sync as the conservative
default. It is not a new full F4 selection gate; Tier 2, non-Linux execution,
and architecture decisions remain deferred.

The audit helper unit tests cover percentile rounding, fixed order rotations,
native/matched concurrency, and timing-free normalization:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
```

## Confirmatory Linux replication

The post-PoC confirmatory run is Linux x86_64/glibc only. It runs three serial
full-profile replicates pinned to CPUs 0-3; Tier 2, non-Linux execution, and
production architecture changes remain out of scope.

From this directory, after verifying that CPUs 0-3 are online and `taskset` is
available:

```sh
root=../results/raw/f4-confirmatory-20260818
binary=target/release/repin-f4-spike

taskset --cpu-list 0-3 python3 scripts/capture_f4_host.py "$root/replicate-01/host.json"
REPINF4_CLEAN_BUILD_MS=<measured-ms> taskset --cpu-list 0-3 "$binary" run --model all --profile full --output "$root/replicate-01"

taskset --cpu-list 0-3 python3 scripts/capture_f4_host.py "$root/replicate-02/host.json"
REPINF4_CLEAN_BUILD_MS=<measured-ms> taskset --cpu-list 0-3 "$binary" run --model all --profile full --output "$root/replicate-02"

taskset --cpu-list 0-3 python3 scripts/capture_f4_host.py "$root/replicate-03/host.json"
REPINF4_CLEAN_BUILD_MS=<measured-ms> taskset --cpu-list 0-3 "$binary" run --model all --profile full --output "$root/replicate-03"

python3 scripts/aggregate_f4_confirmatory.py "$root" "$root/aggregate.json"
```

The aggregation helper validates the 72 cases, 54 measurements, exact case
IDs, sample counts, queue/worker/watch/isolated-worker invariants, and the
byte-equivalent `F4.json`/`F4-report.json` pair for every replicate. Its JSON
preserves raw throughput samples, reports cross-run p95 ranges and hybrid p95
benefits for service and remote workloads, and applies the strict all-replicate
gate. The overall ledger remains Linux PoC recorded/inconclusive even if that
gate passes.
