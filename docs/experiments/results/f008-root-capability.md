# Experiment Result: F-008 — Root-capability opens

```text
Status: complete (Linux PoC; platform expansion deferred)
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#f2-preparation-adversarial-path-manifest-and-open-protocol
Run ID: f008-linux-2026-08-18-rustc-1.97.1
Overall outcome: pass for the Linux capability-open prototype; platform decision deferred until post-PoC expansion
```

## Question and method

Does reopening a walker-provided root-relative path through a retained root
directory capability prevent component-replacement and final-component symlink
races from returning bytes outside the configured root?

The disposable [Rust spike](../f008_root_capability/src/main.rs) compared:

- `canonicalize_then_open`: canonicalize and contain-check, pause, then open
  the absolute path; and
- `root_capability_open`: normalize a root-relative path, open through
  `cap_std::fs::Dir`, disable final-component symlink following through
  `cap-fs-ext`, read with a 1 KiB bound, compare pre/post handle metadata, and
  record a BLAKE3 snapshot hash.

Each case ran 100 times with a deterministic attacker barrier. The raw
machine-readable aggregate is [f008-linux-summary.json](raw/f008-linux-summary.json).

## Result

| Protocol | Case | Observed result |
|---|---|---|
| baseline | `P-NORMAL` | 100/100 in-root reads |
| baseline | `P-TRAVERSAL`, `P-ABSOLUTE`, `P-ESCAPE` | 300/300 rejected |
| baseline | `P-SWAP-COMPONENT` | 100/100 outside-root reads |
| baseline | `P-SWAP-FINAL` | 100/100 outside-root reads |
| capability | `P-NORMAL` | 100/100 in-root reads |
| capability | `P-TRAVERSAL`, `P-ABSOLUTE`, `P-ESCAPE` | 300/300 rejected |
| capability | `P-SWAP-COMPONENT` | 100/100 rejected; no outside bytes |
| capability | `P-SWAP-FINAL` | 100/100 rejected; no outside bytes |

The baseline result confirms that a check followed by an absolute open is
raceable. The capability path never returned the outside sentinel and remained
bounded under both replacement attacks.

The baseline failure is a recorded comparison result, not a blocker for the
dependent foundation work: the capability-path alternative completed the Linux
probe without returning out-of-root bytes. No hard blocker was observed in the
experiment batch.

## Provisional recommendation (decision deferred)

Retain root-relative capability opens as the provisional filesystem-read
protocol for the next evidence pass. Retain canonicalize-then-open only as a
comparison baseline; it cannot satisfy the containment contract under
`P-SWAP` in this harness.

The recommendation is recorded for later plan finalization; it does not accept
the dependency or the complete F2 filesystem experiment. Repeat this report
only during post-PoC platform expansion, retain platform-specific
unsupported/error outcomes, and replace the candidate rather than weakening
the fail-closed contract if any platform can return out-of-root bytes.

## Validation and limitations

- `cargo check --offline --manifest-path docs/experiments/f008_root_capability/Cargo.toml`
- `cargo clippy --offline --release --manifest-path docs/experiments/f008_root_capability/Cargo.toml -- -D warnings`
- `cargo run --offline --release --manifest-path docs/experiments/f008_root_capability/Cargo.toml`
- Linux only in this run; symlink privilege/filesystem semantics on additional
  platforms are intentionally deferred until the fully featured PoC exists.
- The harness uses temporary fixtures and sentinel bytes; it does not claim
  full discovery, ignore, encoding, or reconciliation coverage. Those remain
  in F2 and F-009.
