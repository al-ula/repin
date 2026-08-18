# Experiment Result: F-014 — regex and regex-automata comparison

```text
Status: complete
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#7-f6-direct-regex-and-vcs-adapters
Result revision: working-tree-status: ae8b504320c2e1960da7b0ea46899aa406e2d75fe6e306e70fb68873224bbb44
Run ID: foundation-followup-20260818
Overall outcome: pass
```

## Question and method

Compare `regex` 1.13.1 and `regex-automata` 0.4.16 over the defined safe
syntax, explicit unsupported constructs, exact original byte spans, compile
time, resident-memory deltas for expensive patterns, and 64 KiB scan
checkpoints.

## Results

All eight syntax cases passed: literal, character class, Unicode property,
multiline, alternation, and bounded quantifier were accepted with identical
spans; look-around and backreferences were rejected by both candidates. Three
expensive compile cases recorded candidate-specific timing and RSS deltas.
Both candidates completed the cancellation probe at the 64 KiB checkpoint.

The full timing distributions are in the report measurements. The normalized
comparison artifact was byte-identical on the repeat run.

## Retained evidence

- [JSON report](raw/foundation-followup-20260818-v7/f014-report.json)
- [regex comparison artifact](raw/foundation-followup-20260818-v7/artifacts/f014/regex-comparison.json)

## Limitations and recommendation

The cancellation mechanism is an adapter-owned chunk checkpoint rather than a
claim that either library can asynchronously interrupt an in-progress native
match. The run is Linux x86_64/glibc only and establishes no production memory
threshold. Recommended disposition: `defer` adapter selection and retain both
candidate observations for plan finalization.
