# ADR-017: Verbatim source context packing and blast-radius summaries

```text
Status: accepted contract and capability decision
Date: 2026-08-20
Decision type: context packing and source snippet extraction
Builds on: ADR-003, ADR-005, ADR-006, ADR-014, ADR-016
```

## Decision

Repin adopts verbatim source extraction with 1-indexed line formatting and caller/callee blast-radius header summaries in `ContextBuilder` and context responses, subject to strict byte budgets and working-tree freshness:

1. **Verbatim Line-Numbered Source Packing**: When constructing context snippets for resolved graph nodes with source ranges `[start.line, end.line]`, `ContextBuilder` reads the file content from the working tree (`CapabilityFs`) and emits 1-indexed formatted code lines:
   ```text
   32: pub struct Engine {
   33:     options: EngineOptions,
   34:     store: Option<SqliteStore>,
   35: }
   ```
2. **Blast Radius and Relation Metrics**: Each primary entity snippet is prefixed with a compact summary of its architectural impact (in-degree / incoming caller count and out-degree / outgoing callee count) alongside qualified identifiers and file paths.
3. **Strict Byte Budgeting & Truncation**: All snippet bytes (headers, formatted source lines, and relation summaries) are counted against the caller-supplied `budget_bytes`. If adding a snippet or source line exceeds the budget, packing halts deterministically and marks `truncated: true` in `AssembledContext`.
4. **Layered Degradation**: If source files are unavailable, unreadable, or missing ranges, context packing gracefully degrades to structured metadata summaries with honest provenance and coverage records.

## Rationale

Coding agents (e.g. Gemini, Claude, Cursor) inspecting search results or context neighborhoods require immediately actionable code snippets. Emitting only metadata headers forces agents into secondary, high-latency `read_file` round trips. Packing formatted, line-numbered source lines within a single bounded query maximizes agent token efficiency while strictly adhering to Repin's core "working tree wins" and byte-budgeting invariants.

## Consequences

- `ContextBuilder::assemble_neighborhood` accepts working-tree filesystem access (`&CapabilityFs` or source provider).
- `ContextSnippet` content contains line-numbered code blocks formatted as `<line_no>: <code_line>` when source file reading succeeds.
- Outgoing and incoming relation counts are computed and embedded in the snippet header.
- Output byte budgets are strictly honored, preventing payload bloat and memory exhaustion.

## Required implementation validation

1. Source ranges correctly map to exact 1-indexed lines across single-line, multi-line, and whole-file entities.
2. Truncation flags accurately indicate when byte budgets are exhausted before all candidate entities/neighbors are packed.
3. Fallback to metadata headers works cleanly when files cannot be read from the filesystem.
4. Conformance tests verify no divergence across ASCII, UTF-8, and CRLF line endings.

## Reopen triggers

Reopen this decision if verbatim source loading causes unacceptable latency during context packing on large (>100MB) repositories, or if custom tokenizers are required in place of UTF-8 byte budgets.
