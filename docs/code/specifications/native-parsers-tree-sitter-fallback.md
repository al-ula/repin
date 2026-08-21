# Specification: Language-native parsers with Tree-sitter fallback

```text
Status: accepted normative subsystem specification backing ADR-013
Milestone: I1 — deterministic extraction
Scope: LanguagePack parsing and extraction
Primary recommendation: language-native parser per supported language
Fallback: Tree-sitter, then text-only indexing
```

## 1. Specification

Each supported `LanguagePack` prefers a language-native parser and uses a
version-pinned Tree-sitter grammar as its recovery parser. If neither parser
can produce bounded, trustworthy extraction output, the file remains available
through text-only indexing.

```text
language-native parser
        │ unusable, unsupported, or bounded failure
        ▼
Tree-sitter fallback
        │ unusable or bounded failure
        ▼
text-only indexing
```

Every parser is an adapter behind the existing `LanguagePack` port. Parser
nodes, handles, diagnostics, and library-specific types do not cross into the
core domain.

## 2. Architectural rationale

Language-native parsers expose language-specific syntax and recovery rules
without forcing every language into one lowest-common-denominator tree.
Tree-sitter provides a common fallback shape for incomplete files, syntax not
yet handled by the primary adapter, or a bounded primary-parser failure.

The final text-only fallback preserves the architectural rule that unsupported
or temporarily unparsable source remains discoverable. Parser availability may
reduce structural recall; it must not make a file invisible.

This arrangement also keeps product selection local to each language. A weak
parser candidate for one language does not force the same compromise on every
other `LanguagePack`.

## 3. Normalized extraction boundary

All primary and fallback adapters return the same core-owned output:

```text
ParseOutcome
  parser:       ParserIdentity
  disposition:  primary | fallback | text_only
  facts:        FactBatch
  diagnostics:  Diagnostic[]
  coverage:     Coverage

ParserIdentity
  family:       language_native | tree_sitter
  implementation
  version
  grammarVersion?
  queryVersion?
```

`FactBatch` contains only owned, normalized nodes, relations, attributes,
evidence ranges, skip records, and diagnostics. It cannot retain parser-owned
handles or references.

Every adapter must preserve the normative extraction requirements:

- deterministic output for identical bytes and configuration;
- zero-based half-open byte ranges with validated Unicode-scalar conversions;
- batched extraction rather than repeated cross-boundary node access;
- bounded depth, memory, work, diagnostics, and cancellation latency;
- explicit partial-parse and skipped-content diagnostics; and
- stable ordering before facts enter the transactional update pipeline.

## 4. Fallback semantics

Fallback is a controlled disposition, not an attempt to merge competing parse
trees.

- A primary parse that contains recoverable syntax errors may still be usable.
  Diagnostics alone do not trigger fallback.
- The primary adapter explicitly classifies its output as usable, unsupported,
  failed, or resource-limited.
- Tree-sitter runs only when the primary result is unusable for the required
  extraction contract.
- Facts from primary and fallback parsers are not merged for one file revision.
  Mixing them would make ownership, identity, removal, and reproducibility
  ambiguous.
- The selected parser identity and fallback disposition are recorded with the
  file's extraction evidence and exposed through diagnostics/status.
- If both structured parsers fail, previous file-owned facts are removed by the
  normal replacement protocol and the file is indexed as text-only.

The engine may optionally run both parsers in validation tooling, but that is
comparison evidence and never normal production extraction.

## 5. Versioning and invalidation

Parser implementation, parser version, grammar version, extraction-query
version, and normalization rules participate in the language pack's version
record.

A change to any component that can alter normalized facts invalidates affected
file-owned extraction output. Re-extraction replaces the prior facts
transactionally; revisions are not reused. Snapshots remain separate per
parser binding because primary and fallback trees are not assumed equivalent.

## 6. Implementation-time package selections

Product package selection occurs during the respective language pack milestone:

| Language family | Native primary candidates | Common fallback |
| --- | --- | --- |
| Rust | rust-analyzer parser family; `syn` where complete-input AST is sufficient | pinned Tree-sitter Rust grammar |
| TypeScript and JavaScript | Oxc; SWC parser family | pinned Tree-sitter TypeScript/JavaScript grammars |
| Markdown | Rust-native event or AST parsers with source-position support | pinned Tree-sitter Markdown grammar where suitable |

The implementation task verifies:

- behavior on incomplete and erroneous source;
- exact byte-range and source-position support;
- batch traversal or query facilities;
- supported syntax and dialects;
- cancellation and resource bounding;
- deterministic output and serialization shape;
- Rust API ergonomics and native build surface;
- release cadence, compatibility policy, licensing, and dependency footprint; and
- the cost of pinning and upgrading parser plus grammar assets.

## 7. Acceptance criteria

Implementation profiles derived from this architecture are acceptable when:

- both adapters map into one normalized `FactBatch` contract;
- fallback triggers and provenance are externally observable;
- malformed, incomplete, unsupported, and resource-limited inputs have bounded
  dispositions;
- upgrades produce reviewable per-binding snapshot changes;
- switching between primary, fallback, and text-only removes stale owned facts;
  and
- absence or failure of structured parsing preserves direct and lexical access.

## 8. Non-decisions

This specification does not require every supported language to have a native
primary parser before implementation begins. It does not change the range contract
in [ADR-006](../decisions/ADR-006-extraction-and-ranges.md) or the transactional
replacement rules in [Incremental Updates](../incremental.md).
