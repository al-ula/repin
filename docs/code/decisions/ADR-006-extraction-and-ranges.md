# ADR-006: Preserve deterministic extraction and range semantics

```text
Status: accepted contract decision; implementation profiles selected by ADR-013 and ADR-014
Date: 2026-08-19
Decision type: extraction and evidence coordinates
Supersedes: none
```

## Decision

Extraction output is deterministic for a fixed input, grammar/query pack, and
capture order. Public ranges retain original byte offsets and normalized
1-based line/Unicode-scalar positions. Invalid UTF-8 maps one replacement
scalar per maximal invalid run. Parser and isolated-worker cancellation must
stop without publishing a partial fact batch.

The sparse-checkpoint line-index representation is selected by ADR-014. Its
stride remains a private tuning choice. The native-primary/Tree-sitter-fallback
architecture is selected by ADR-013; concrete parser packages are selected
when their corresponding `LanguagePack` is implemented.

## Evidence

F-017 passed repeated byte-identical captures, the full range oracle across
ASCII, UTF-8, combining marks, tabs, CRLF, invalid bytes, long lines,
boundaries, and empty input, plus native and isolated-worker cancellation.
F-004 found checkpoint, full-map, and line-scan representations equivalent and
identified checkpoints as a provisional internal baseline.

## Consequences

- Clients do not need to normalize parser output or recalculate coordinates.
- Exact byte evidence remains available for slicing while character positions
  remain suitable for display.
- Grammar/query versioning remains required. Concrete parser packages are
  deferred implementation choices rather than plan-level open decisions.
- Byte-to-position conversion uses the sparse-checkpoint profile in ADR-014.
