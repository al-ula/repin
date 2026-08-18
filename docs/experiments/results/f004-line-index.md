# Experiment Result: F-004 — Byte-to-character line index

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#f-004-initial-prototype-result-2026-08-18
Run ID: f004-linux-2026-08-18-rustc-1.97.1
Overall outcome: pass for prototype equivalence; inconclusive for final selection
```

## Question and result

F-004 compares three private representations for converting a byte offset into
a Unicode-scalar line/column position:

1. a complete per-byte scalar map;
2. scalar checkpoints every 64 bytes plus bounded decoding; and
3. decoding from the nearest line start.

The std-only prototype at [`f004_line_index.rs`](../f004_line_index.rs)
returned identical positions at every generated scalar boundary for ASCII,
UTF-8, and invalid-byte fixtures. It exercised 80-byte and 4,096-byte lines,
sequential, deterministic-random, and hot lookup locality, and a 256 KiB
workload.

Representative UTF-8 output for 4,096-byte lines is below. Lookup timing is
6,144 lookups (2,048 offsets repeated three times); memory is an estimate of
the index allocations, not process RSS.

| Shape | Build | Random lookup | Hot lookup | Estimated memory |
|---|---:|---:|---:|---:|
| Full map | 992 µs | 204 µs | 50 µs | 1,049,100 B |
| 64-byte checkpoints | 586 µs | 1,020 µs | 319 µs | 65,752 B |
| Line scan | 153 µs | 16,346 µs | 394 µs | 520 B |

Invalid-byte density changed absolute timings but did not change the ordering
or the equivalence result.

## Provisional recommendation (decision deferred)

Use the checkpoint representation as the provisional internal baseline. It
keeps scan work bounded and used roughly one-sixteenth of the full-map memory
in the long-line fixture, while avoiding the long-line random-access cost of a
line-only scan. Keep the stride private and versioned as an implementation
parameter; it is not part of the public range contract.

This is not a final production selection. The recommendation is recorded for
later plan finalization and does not accept or reject a representation. Before
plan finalization, rerun with 32/64/128-byte strides, larger real corpora, Tier
2 builds, and cancellation instrumentation. A full map may still be useful for
a deliberately bounded hot-file cache, but it should not be treated as the
default allocation for every file based on this run alone.

## Evidence and limitations

- Command: `rustc --edition=2024 -O docs/experiments/f004_line_index.rs -o /tmp/repin-f004 && /tmp/repin-f004`.
- Environment: Linux x86_64, AMD Ryzen 7 5825U, `rustc 1.97.1`.
- The prototype is disposable and has no parser or storage dependency.
- The result measures representation trade-offs, not parser throughput or a
  cross-platform performance guarantee.
