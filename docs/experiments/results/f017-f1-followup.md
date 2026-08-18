# Experiment Result: F-017 — F1 evidence follow-up

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#2-f1-tree-sitter-extraction-substrate
Result revision: working-tree-status: ae8b504320c2e1960da7b0ea46899aa406e2d75fe6e306e70fb68873224bbb44
Run ID: foundation-followup-20260818
Overall outcome: pass
```

## Question and method

Does the reviewed `f1-query-v1` pack set produce deterministic captures and
exact range positions, including malformed/invalid-byte boundaries, while
parser cancellation and isolated-worker fallback remain bounded?

From `docs/experiments/foundation_spike`:

```sh
cargo run --release --locked --offline \
  --features gix-adapter,sniff-adapter \
  --bin repin-foundation-followup -- \
  run-all --output ../results/raw/foundation-followup-20260818-v7
```

The run used the pinned tree-sitter core and Rust, Markdown, TypeScript, and
JavaScript grammars. Query text is SHA-256 recorded in the query manifest;
capture output is sorted by the versioned byte/order tuple and then deduped.

## Results

| Requirement | Evidence | Outcome |
|---|---|---|
| Reviewed query packs and reproducible capture order | Four language capture artifacts; pre/post-dedup counts retained | pass |
| Exact ranges for ASCII, UTF-8, combining marks, tabs, CRLF, invalid UTF-8, long lines, boundaries, and empty input | `range-oracle.json` and report cases `F017-RANGE-*` | pass |
| Invalid bytes map one replacement scalar per maximal invalid run | `F017-RANGE-R-INVALID`; normalized text `a�b` | pass |
| Native parser cancellation | `F017-PARSER-CANCELLATION` | pass |
| Isolated worker cancellation/reap without a fact batch | `F017-ISOLATED-WORKER` | pass |

The repeated capture sequences were byte-identical. The normalized query,
capture, range, and manifest artifacts were byte-identical on a repeat run.

## Retained evidence

- [JSON report](raw/foundation-followup-20260818-v7/f017-report.json)
- [query manifest](raw/foundation-followup-20260818-v7/artifacts/f017/query-manifest.json)
- [Rust captures](raw/foundation-followup-20260818-v7/artifacts/f017/captures-rust.json)
- [Markdown captures](raw/foundation-followup-20260818-v7/artifacts/f017/captures-markdown.json)
- [TypeScript captures](raw/foundation-followup-20260818-v7/artifacts/f017/captures-typescript.json)
- [JavaScript captures](raw/foundation-followup-20260818-v7/artifacts/f017/captures-javascript.json)
- [range oracle](raw/foundation-followup-20260818-v7/artifacts/f017/range-oracle.json)

## Limitations and recommendation

This is Linux x86_64/glibc evidence only. It does not select a grammar,
query-pack policy, or production parser representation. Platform expansion and
plan-finalization decisions remain deferred.

Recommended disposition: `defer` the production decision; retain the evidence
and reopen only for the explicitly scoped platform or grammar follow-up.
