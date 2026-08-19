# ADR-016: Agent-oriented inspection and change-review context

```text
Status: accepted contract and capability decision
Date: 2026-08-19
Decision type: agent inspection and review navigation API
Builds on: ADR-005, ADR-006, ADR-014
```

## Decision

Repin adopts a bounded, agent-oriented inspection profile composed of:

1. `inspectFile(InspectFileRequest) -> Result<FileInspection>` for structured
   file outlines, symbol summaries, import/export relationships, and recommended
   entities without reading entire source bodies;
2. `EntityRef::AtPosition` to resolve a 1-based source position or byte offset
   to an exact or smallest enclosing entity;
3. `reviewContext(ReviewContextRequest) -> Result<ReviewContext>` for composing
   working-tree or revision changes, transitive graph impact, and budgeted
   context into a single deterministic review payload; and
4. Identifier-aware lexical sub-tokenization for the symbol retrieval channel.

All operations follow Repin's core invariants:

- **Working tree wins**: Evidence bodies are reread directly from current
  working tree bytes; metadata from the graph is verified against current
  content.
- **Layered degradation**: When graph data is absent, `inspectFile` falls back
  to syntax-only or text-only metadata with explicit `Coverage` and
  `OmissionReport` records.
- **Deterministic bounds**: Operations are non-pageable in v1, respect output
  and token budgets, use stable sort keys, and report truncation honestly.

The detailed contract and acceptance criteria are recorded in the
[agent inspection proposal](../proposals/agent-inspection-and-review-context.md).

## Rationale

Agents navigating codebases currently face an efficiency gap between broad
search queries and expensive whole-file reads. Providing a structured file
outline (`inspectFile`) and line-to-entity resolution (`AtPosition`) allows
agents to inspect module shapes and fetch exact enclosing bodies with minimal
token overhead. Combining changed files, reverse impact, and context
(`reviewContext`) provides a deterministic one-shot review payload without
requiring clients to implement complex graph traversal logic.

## Consequences

- `inspectFile` and `reviewContext` become part of the public `ProjectClient`
  API contract.
- `EntityRef` is extended with the `AtPosition` variant.
- Symbol channel indexing normalizes identifiers (camelCase, snake_case,
  kebab-case, dotted names) at the lexical adapter boundary.
- Context budgeting and omission semantics apply to review bundles.

## Required implementation validation

1. `AtPosition` resolution matches extraction ranges across ASCII, Unicode,
   combining marks, CRLF, and pathological long lines.
2. Boundary positions (whitespace/comments between entities) resolve to the
   containing scope or return an explicit `NO_ENCLOSING_ENTITY` outcome.
3. Pathological and minified files respect symbol caps and output budgets
   without memory exhaustion.
4. `reviewContext` correctly resolves uncommitted working-tree changes when
   `changesSince` is omitted.
5. Invalidation or lack of graph facts produces deterministic syntax-only or
   text-only fallbacks with accurate provenance.

## Reopen triggers

Reopen this decision if the inspection profile introduces significant
extraction latency, if identifier sub-tokenization degrades FTS5 retrieval
precision on fixed benchmarks, or if agents require mutating editor operations
that violate the read-only retrieval engine boundary.
