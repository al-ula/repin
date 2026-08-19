# Specification: Agent-oriented inspection and change-review context

```text
Status: accepted normative subsystem specification backing ADR-016
Milestones: I1 — deterministic engine; I4 — retrieval quality
Scope: bounded agent-facing navigation over the graph and working tree
Primary recommendation: inspection funnel plus change-review context
```

## 1. Specification

Repin provides an agent-oriented inspection profile composing
symbol search, graph exploration, impact analysis, and context capabilities into bounded navigation
steps:

```text
symbol search
      │
      ▼
file inspection / module outline
      │
      ├── exact symbol or enclosing-entity read
      └── bounded change-review context
```

The profile provides:

1. a bounded `inspectFile` operation for syntax and graph-backed file shape;
2. an `AtPosition` source-position entity reference for resolving the smallest enclosing entity; and
3. a `reviewContext` composition starting from changed files or revisions, following bounded impact, and returning budgeted working-tree evidence.

## 2. API contracts

```text
inspectFile(request: InspectFileRequest, call?: CallOptions)
  -> Result<FileInspection>

InspectFileRequest
  path:             Path
  budget?:          OutputBudget

FileInspection
  path:             Path
  language?:        LanguageId
  artifactClass:    ArtifactClass
  symbols:          SymbolSummary[]
  imports?:         RelationshipSummary[]
  exports?:         RelationshipSummary[]
  recommended?:     EntityRef[]
  coverage:         Coverage
  provenance:       ProvenanceSummary
```

`SymbolSummary` contains entity identity, kind, signature/summary attributes, source range, export/visibility attributes where known, and bounded relationship counts. It may include ranked `recommended` entities without embedding every body or caller in the default response.

The entity reference contract supports source-position resolution:

```text
EntityRef
  = ById       { id: EntityId }
  | ByName     { name: Text, kind?: NodeKind, pathHint?: Path }
  | AtPosition { path: Path, position: Position,
                 preference?: exact | smallest_enclosing }
```

`AtPosition` resolves against selected, currently known facts and current file bytes. It returns explicit outcomes for no enclosing entity, ambiguous entities, unsupported language structure, and stale evidence. It never silently guesses across files.

The existing `context` operation supplies the exact body read once an entity is resolved:

```text
context(
  entities: [AtPosition(...) | ById(...)],
  strategy: [exact],
  budget:    OutputBudget
)
```

The review context composition operates over changes, impact, and context:

```text
reviewContext(request: ReviewContextRequest, call?: CallOptions)
  -> Result<ReviewContext>

ReviewContextRequest
  changesSince?: Revision       // omitted: uncommitted working-tree changes
  paths?:        PathPattern[]
  maxDepth:      Count
  include?:      { callers?, tests?, docs?, config?, dependencies? }
  budget:        OutputBudget

ReviewContext
  changed:       Entity[]
  impact:        ImpactGroup[]
  fragments:     ContextFragment[]
  recommended:   EntityRef[]
  omitted:       OmissionReport
  coverage:      Coverage
```

`reviewContext` is a composition over `changesSince`, `impact`, and `context`; it strictly preserves existing depth, output, redaction, freshness, and coverage rules.

## 3. Architectural rationale

Repin establishes deterministic foundations: symbol and graph-aware search, bounded impact, explainable ranking, source ranges, and budgeted context assembly ([Retrieval](../retrieval.md), [Public API](../api.md)). This specification bridges these primitives into coherent agent workflows.

The design strictly preserves the working-tree rule: inspection metadata originates from the graph, but exact source bodies and evidence are re-read directly from current working-tree bytes with full freshness and redaction enforcement.

## 4. Identifier-aware lexical profile

The symbol channel normalizes and tokenizes compound identifiers into their complete spelling and sub-components across camelCase, PascalCase, snake_case, kebab-case, dotted names, and digit boundaries.

This is implemented at the SQLite FTS5 adapter boundary by indexing a derived identifier-token field without requiring an external search engine or modifying node identity. Ranking contributions remain visible in `RankExplanation`.

## 5. Degradation and bounds

The profile degrades in layers:

- **no graph:** return syntax-only inspection when the language pack provides it;
- **no structured parser:** return text/file metadata with explicit coverage reduction;
- **stale/lagging graph:** identify the graph revision and use current bytes for returned evidence;
- **missing impact data:** return changed entities and direct context with an explicit omission record;
- **oversized entities or context:** return bounded slices with an `OmissionReport`; never silently truncate;
- **cancellation/deadline:** publish no partial mutation and return bounded cancellation outcomes.

Every operation is non-pageable in v1, uses semantic and hard output budgets, and reports coverage, provenance, freshness, and truncation in the standard result envelope.

## 6. Implementation sequence

### I1 — Deterministic inspection primitives

- Implement `AtPosition` resolution using the accepted parser and line-index profiles.
- Implement `inspectFile` with syntax-only fields and stable ordering.
- Leverage `context(strategy: exact)` for symbol/enclosing reads.
- Add test fixtures for nested entities, duplicate names, callbacks, invalid UTF-8, and unsupported languages.

### I4 — Graph-enriched review context

- Add callers/dependents and relationship summaries to inspection.
- Implement `reviewContext` over changed revisions and explicit paths.
- Add deterministic recommendation heuristics based on export status, fanout, complexity, and artifact class.
- Validate token budgets, latency, and omission honesty on standard test repositories.

## 7. Acceptance criteria

Implementation validation demonstrates:

- `AtPosition` resolution matches extraction ranges and rejects ambiguous or stale boundaries;
- inspection output is deterministic, bounded, stable-key ordered, and functional without a graph;
- exact reads always come from current working-tree bytes and carry evidence;
- review context respects depth, output, redaction, and cancellation budgets;
- impact and inspection provenance distinguish graph-backed, syntax-only, and heuristic sections;
- unsupported languages, stale graphs, and omitted impact remain visible in coverage; and
- ranking recommendations retain explainable reasons.

## 8. Non-decisions

This specification does not make Repin a language server, compiler, linter, formatter, or mutating editor. It does not add mutating editor operations or AST rewrites to the core engine.
