# ADR-013: Prefer language-native parsers with Tree-sitter fallback

```text
Status: accepted extraction architecture; parser packages deferred to implementation
Date: 2026-08-19
Decision type: LanguagePack parsing strategy
Builds on: ADR-006
```

## Decision

Each `LanguagePack` prefers a language-native parser, uses a version-pinned
Tree-sitter grammar when the primary result is unusable, and finally degrades
to text-only indexing. One file revision uses one structured parser result;
primary and fallback facts are never merged.

All parser adapters return the same core-owned `FactBatch`, range,
diagnostic, coverage, and provenance types. Parser-owned handles and types do
not cross the `LanguagePack` boundary. Parser, grammar, query, and normalization
versions participate in extraction invalidation.

Concrete packages for Rust, TypeScript/JavaScript, and Markdown are selected
when those language packs are implemented. They do not block plan
finalization. The complete fallback and versioning rules are recorded in the
[parser proposal](../proposals/native-parsers-tree-sitter-fallback.md).

## Rationale

Language-native parsers can expose language-specific structure while
Tree-sitter supplies a common recovery path for incomplete or unsupported
syntax. The text-only final fallback preserves discoverability when structured
parsing is unavailable.

## Consequences

- Parser selection is local to each language pack.
- Fallback use is explicit in diagnostics and evidence provenance.
- Recoverable primary-parser diagnostics do not automatically trigger
  fallback; the adapter must classify the result as unusable.
- Changing parser identity or extraction assets invalidates affected facts.

## Reopen triggers

Reopen the architecture only if a language cannot provide bounded normalized
facts through either path, or if running a fallback parser materially violates
the accepted resource or distribution profile.
