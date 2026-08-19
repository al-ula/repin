# ADR-010: Use `regex` for initial direct regex search

```text
Status: accepted implementation choice for the initial Linux PoC
Date: 2026-08-19
Decision type: direct-search adapter
Builds on: ADR-005
```

## Decision

Repin's initial direct-regex adapter uses the Rust `regex` crate. The exact
crate version and feature set are pinned in the implementation lockfile when
the direct-search crate is created.

The adapter implements the public v1 syntax already defined in
[Retrieval](../retrieval.md#direct-regex-contract). It returns overall-match
byte spans, applies explicit pattern and compiled-size limits, bounds the
searched content, and participates in the request's deadline and cancellation
protocol. Unsupported constructs are rejected as `INVALID_QUERY`; they are not
silently reinterpreted.

`regex-automata` remains an eligible internal escalation if implementation
evidence demonstrates that Repin needs lower-level engine selection or resource
controls unavailable through `regex`. Such an internal change must preserve the
same public syntax and result contract.

## Rationale

The documented high-level API directly provides the behavior required by the
initial adapter:

- regular-language matching without look-around or backreferences;
- Unicode-aware string matching and byte-oriented matching where required;
- exact byte offsets for matches;
- fallible compilation with a configurable compiled-size limit; and
- a substantially smaller adapter surface than selecting and coordinating
  individual automata engines.

The `regex` implementation already uses the `regex-automata` meta engine
internally. Choosing the high-level crate therefore avoids committing Repin to
manual DFA/NFA selection while retaining a compatible lower-level path if
measured requirements justify it.

This decision is based on public documentation and theoretical conformance to
ADR-005. It does not claim measured Repin-specific performance or cancellation
latency.

## Consequences

- Direct regex search has a concrete initial adapter and no longer blocks plan
  finalization.
- The public regex dialect remains owned by Repin, not inferred from every
  private feature the crate may support.
- Pattern length, compiled size, input size, result count, and output bytes need
  explicit Repin limits even when the crate has its own defaults.
- A single blocking search call has no general external cancellation callback.
  The adapter must bound individual work units or isolate them so request
  cancellation remains observable at the ADR-005 safe points.
- Lexical FTS5 search, file selection globs, parsing, and graph queries do not
  use this adapter.

## Required implementation validation

1. Every advertised v1 construct produces the specified matches and byte
   ranges; every excluded construct returns `INVALID_QUERY`.
2. Unicode, CRLF, empty matches, invalid input bytes, long lines, anchors, and
   multiline behavior match the public contract.
3. Pattern, compiled-size, haystack, result-count, and output limits fail or
   truncate with explicit bounded outcomes.
4. Cancellation and deadlines remain responsive for adversarial patterns and
   maximum-size permitted inputs.
5. Stable traversal and result ordering are independent of filesystem
   enumeration order.

## Reopen triggers

Reopen the adapter selection if `regex` cannot meet the accepted cancellation,
resource, byte-input, or exact-span contract without weakening public behavior.
In that case, evaluate `regex-automata` behind the same adapter before changing
the public regex dialect.

## Not decided

This ADR does not select the VCS adapter, file-discovery glob implementation,
line-index representation, parser packages, or final dependency/MSRV policy.
