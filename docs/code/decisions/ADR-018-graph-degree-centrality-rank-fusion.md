# ADR-018: Graph degree centrality in deterministic rank fusion

```text
Status: accepted contract and capability decision
Date: 2026-08-20
Decision type: search ranking and graph centrality signal
Builds on: ADR-005, ADR-008, ADR-009, ADR-016
```

## Decision

Repin integrates graph degree centrality (in-degree fan-in) into the deterministic rank fusion pipeline in `DeterministicRanker`:

1. **In-Degree Signal Calculation**: For each candidate node returned across retrieval channels (symbol exact, prefix, substring, path, FTS5 BM25 lexical), the engine computes the incoming edge count (in-degree):
   $$\text{in\_degree}(u) = |\{ (v, u) \in E \}|$$
2. **Normalized Centrality Scoring**: The centrality bonus is calculated relative to the maximum in-degree observed among the candidate set:
   $$\text{score}_{\text{centrality}} = \min\left(1.0, \frac{\text{in\_degree}(u)}{\max(1, \max_{w \in C} \text{in\_degree}(w))}\right) \times 0.15$$
3. **Deterministic Tie-Breaking & Explanations**: The centrality bonus is added to the total rank score and recorded explicitly in `RankExplanation` under the signal name `"graph_degree_centrality"`. If scores are tied, ordering is deterministically resolved by the lexicographical byte comparison of `NodeId`.
4. **Degradation**: If graph edge data is absent, zeroed, or disabled, the centrality score is omitted (0.0) without disrupting deterministic symbol or lexical scoring.

## Rationale

In real-world codebases, generic or common names (such as `Engine`, `Context`, `State`, `Config`, `Error`) are shared across numerous localized internal helper variables, test fixtures, and core architectural types. Boosting candidates proportional to their graph fan-in (reference and caller count) provides a significant improvement in Precision@1 and Mean Reciprocal Rank (MRR) for architectural queries without introducing non-deterministic heuristics or heavyweight machine learning models.

## Consequences

- `ReadView` trait in `repin-core` is extended with in-degree query support (`incoming_edge_count`).
- `SqliteReadView` implements efficient indexed in-degree counting over `edge_claims`.
- `DeterministicRanker::rank_fusion` accepts candidate in-degrees and factors them into total score calculation.
- `RankExplanation` records transparent, auditable evidence for centrality contributions.

## Required implementation validation

1. Centrality bonus strictly boosts architectural hub nodes above isolated local variables of the same name.
2. Nodes with zero in-degree receive zero centrality bonus.
3. Stable tie-breaking holds across repeated queries and platform restarts.
4. Latency of in-degree retrieval remains negligible (<2ms) via SQLite indexed lookups.

## Reopen triggers

Reopen this decision if in-degree calculations noticeably degrade query latency on graphs with >500k edges, or if PageRank / HITS algorithm is proven necessary over localized degree centrality.
