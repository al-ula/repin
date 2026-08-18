# Experiment Result: F1 — Tree-sitter extraction substrate

```text
Status: complete (Tier 1 evidence retained; overall inconclusive)
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#2-f1-tree-sitter-extraction-substrate
Run ID: foundation-tier1-20260818
Overall outcome: inconclusive
```

## Result

The pinned Rust, Markdown, TypeScript, and JavaScript bindings all parsed the
same fixture deterministically across five repeated capture batches. The
malformed Rust fixture produced a bounded partial tree, and the 128 KiB
long-line fixture completed without an unbounded operation. The raw feature-run
report records 18, 7, 15, and 10 captures respectively.

The run used the wildcard query `(_) @node` as a substrate probe. It did not
yet exercise the production query-pack manifest, parser cancellation, or the
full invalid-UTF-8 range oracle. No hard blocker was observed; these are Linux
PoC evidence gaps, not normative-contract failures. Additional-platform
build/behavior work is intentionally post-PoC.

## Provisional recommendation (decision deferred)

Keep the parser and grammar candidates provisional. The deterministic capture
and build evidence is sufficient to continue the foundation work, not to make
an acceptance, rejection, or implementation-default decision about a language
pack or range/cancellation conformance.

This recommendation is recorded for later plan finalization. The experiment
does not accept or reject a candidate.

## Required follow-up

- run the reviewed query packs and compare pre-dedup capture sequences;
- verify exact Unicode-scalar, CRLF, and invalid-byte positions;
- exercise parser timeout/isolated-worker behavior; and
- defer additional-platform build/behavior validation until the fully featured
  Linux PoC is complete.

## Evidence

- [feature-run batch report](raw/foundation-tier1-features-20260818/batch.json)
- [feature-run F1 report](raw/foundation-tier1-features-20260818/F1-report.json)
- [feature-run manifest](raw/foundation-tier1-features-20260818/manifest.json)
- [spike workspace](../foundation_spike/)
