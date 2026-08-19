# Retrieval

Finding things, ordering them, and assembling material for a consumer. Three separable concerns, kept separate because they fail and improve independently.

```text
retrieve   gather candidates from several channels
rank       order them deterministically, with reasons
context    assemble bounded material for a consumer
```

## 1. Channels

```text
query
 │
 ├─ lexical    text and phrase matching
 ├─ symbol     declaration lookup by name
 ├─ structural path, kind, and class filtering
 ├─ graph      proximity to known entities
 └─ semantic   embedding similarity            (optional)
 │
 ▼ merge ──> rank ──> cutoff ──> rerank (optional) ──> results
```

Each channel is independently available or absent. A query uses whichever exist, and the result reports which contributed ([Results and Evidence §1](results.md#1-envelope)). Absence of a channel narrows recall; it never fails a query.

| Channel | Requires | Absent |
| --- | --- | --- |
| lexical | lexical port, or direct scan | never absent |
| symbol | a pack with `symbols`, or graph | falls back to text |
| structural | file nodes, or filesystem | never absent |
| graph | a usable graph | omitted |
| semantic | vector port and embeddings | omitted |

The **symbol channel** employs identifier-aware lexical tokenization: identifiers are normalized into their complete spelling as well as constituent parts across camelCase, PascalCase, snake_case, kebab-case, dotted names, and digit boundaries. This expands recall for partial identifier queries while retaining exact-match priority.

**Semantic retrieval complements deterministic retrieval; it does not replace it.** A semantic channel that returns plausible-looking but wrong entities is worse than no semantic channel, because deterministic channels' precision gets diluted. Semantic hits enter the same merge as everything else and must earn their position.

## 2. Merge

Candidates from several channels are merged before ranking, not concatenated after.

- Identity is by node id, so the same entity found by three channels is one candidate with three match signals — not three results.
- Each contributing signal is retained with its channel and channel-local score. This is what makes ranking explainable.
- Merge is order-independent: channel completion order does not affect the outcome.
- A candidate matched by several channels is usually a better answer than one matched by a single channel, and ranking should reflect that. Multi-channel agreement is a strong signal.

## 3. Deterministic ranking

Ranking is a pure function of retrieved signals and graph structure. **No model participates.**

Signals:

```text
match quality      exactness, completeness, position of match
symbol match       exact name > qualified suffix > prefix > fuzzy
path relevance     proximity to query terms, depth, directory conventions
kind preference    query-appropriate node kinds
artifact class     class preference for the query type
graph proximity    hops from an anchor entity
relation relevance which relation kinds connect it
recency            revision of last change, weakly weighted
source priority    configured root or path preference
channel agreement  how many channels found it
```

Requirements:

1. **Deterministic.** Same graph plus same query yields the same order. Ties break on a stable key, never on iteration order — non-deterministic ranking makes every regression test flaky and every bug report unreproducible.
2. **Explainable.** Every result can state why it ranked where it did.
3. **Bounded.** Ranking cost is bounded by candidate count, not graph size.
4. **Tunable but not arbitrary.** Weights are configurable; the signal set is not per-query.

### Explanations

```text
RankExplanation
  score:   Score
  reasons: Reason[]

Reason
  signal:       SignalKind
  contribution: Score
  detail?:      Text
```

Example:

```text
score 0.91
  exact symbol match          +0.40
  two hops from anchor        +0.25   via imports, calls
  matching document section   +0.15
  preferred artifact class    +0.11
```

Explanations make retrieval quality debuggable with no model in the loop, and they are how a ranking regression gets diagnosed rather than argued about. They are also directly useful to a consumer deciding how much to trust a result.

## 4. Filters

Filters constrain candidates; they are not ranking signals.

```text
Filters
  roots?:          RootId[]
  paths?:          PathPattern[]
  exclude?:        PathPattern[]
  languages?:      LanguageId[]
  artifactClasses?: ArtifactClass[]
  nodeKinds?:      NodeKind[]
  relations?:      EdgeKind[]
  derivation?:     Derivation[]
  changedSince?:   Revision
```

Rules:

- An unsupported filter returns `CAPABILITY_UNSUPPORTED` **with the supported set**, so a caller corrects itself in one round trip.
- Filters are applied before ranking, so limits count post-filter candidates.
- `derivation` lets a caller demand deterministic facts only — the mechanism by which a cautious consumer opts out of heuristics and inference entirely.
- Filters never widen selection. An excluded path stays excluded ([Safety and Data Handling §2](safety.md#2-exclusions)).

## 5. Graph operations

Four operations over the graph, each bounded, each reporting coverage.

### Entity resolution

Name to entity, with explicit ambiguity ([Results and Evidence §4](results.md#4-ambiguity)). Never silently chooses among materially different candidates. Accepts a scope hint to disambiguate.

### Neighbors

Immediate typed relationships around an entity, filterable by direction, relation kind, and neighbor kind. Bounded by count per relation kind so one highly-connected relation cannot crowd out the rest.

### Trace

Bounded paths between two entities.

- Depth and path count are bounded, with conservative defaults and a hard maximum.
- Returns **ordered paths**, one edge per step — not an unstructured subgraph dump. A subgraph is not an answer to "how does A reach B."
- Distinguishes three outcomes that callers act on differently: endpoint unresolved, endpoints resolved with no path, and path search truncated.
- Only traverses relation kinds marked transitively meaningful in the registry ([Graph Model §5](graph-model.md#5-kind-registries)). Chaining relations that do not compose produces paths that exist in the graph but not in reality.

### Impact

Reverse-reachable candidates from a target.

- Depth-bounded, count-bounded, grouped by distance and artifact class.
- Results are **impact candidates, not consequences.** This is a language requirement, not a caveat: an automated consumer told "these are affected" will act as though they are.
- Never claimed exhaustive. Dynamic dispatch, reflection, generated code, and unresolved references all hide real impact, and `coverage` must say so.
- Includes non-code artifacts — tests, documentation, configuration, schemas — because those are frequently the impact a caller most needs and least expects.

## 6. Coverage in answers

Every graph answer carries coverage ([Results and Evidence §1](results.md#1-envelope)), computed from real inputs rather than asserted:

- unresolved-reference ratio in the relevant region
- skipped files in the relevant region
- languages present but unsupported or partially supported
- index staleness relative to the working tree

A known-lagging lexical index is bypassed. Direct search over current selected files substitutes for lexical retrieval where the requested mode supports it, and graph channels are merged normally. The result always warns that lexical retrieval was bypassed. If the direct scan completely covers the scope, coverage may remain `complete`; if limits, skips, timeout, or unsupported direct semantics leave gaps, status is `partial` and coverage is `partial`. Stale lexical snippets are never presented as current evidence.

**An absent result in a partially-covered graph is not evidence of absence.** `NO_PATH_FOUND` with `coverage: partial` and `coverage: complete` are different claims, and only one supports a conclusion. Reporting them identically invites exactly the wrong inference.

## 7. Optional reranking

A model may reorder the top candidates. It may not retrieve.

```text
retrieve 50–100
     ↓ deterministic rank
     ↓ cutoff to 20–40
     ↓ rerank                (optional)
     ↓ return ~10
```

- Reranking operates on an already-ranked deterministic candidate set. It cannot introduce entities the deterministic path did not find, which bounds its blast radius.
- Absent, unavailable, or failed: deterministic order stands and the result is `ok`, not `partial`. A failed optional improvement is not a degraded answer.
- Reranked results retain deterministic explanations, plus a note that reranking was applied.
- Reranking never changes evidence, provenance, or derivation.
- Bounded by a deadline. It MUST NOT delay results indefinitely; on timeout the deterministic order returns.

## 8. Context construction

Distinct from search, and the operation where a budgeted consumer gains most.

```text
search(query)            -> ranked entities with evidence
context(nodes, budget)   -> assembled, ordered, deduplicated material
```

`search` answers "what is relevant." `context` answers "what should be put in front of a consumer with a fixed budget." These require different logic: the second needs node kinds, graph proximity, and content sizes — information a client does not have and should not have to compute.

### Strategies

| Strategy | Assembles |
| --- | --- |
| `exact` | only the requested entities |
| `neighborhood` | entities plus immediate relationships |
| `dependency` | entities plus what they depend on |
| `dependents` | entities plus what depends on them |
| `call_chain` | callable entities along call paths |
| `definition_first` | declarations before uses |
| `diversity` | spread across files, kinds, and classes |
| `budget_packed` | maximizes useful material within a budget |

Strategies compose. Available strategies are reported in capabilities so a caller does not guess.

### Rules

1. **Budget is respected exactly.** Over-budget output forces the consumer to truncate blindly, discarding the ordering the engine just computed.
2. **Deduplicate overlapping regions.** Two entities in one file yield one merged region, not two overlapping excerpts.
3. **Order for consumption.** Definitions before uses; containers before contents; highest-ranked first. A truncated context must still lead with its most useful material.
4. **Label every fragment** with root-relative path and range. An unlabeled fragment cannot be verified against the working tree.
5. **Read from the working tree**, not from stored copies. Context must reflect current content, and this is what makes "the working tree wins" ([Architecture §1](architecture.md#1-two-capabilities-one-product)) true in practice.
6. **Report what was omitted.** Truncation is always visible.
7. **Apply redaction** ([Safety and Data Handling §3](safety.md#3-redaction)). Context is output.

Where a budget is expressed in units the engine cannot compute exactly—model tokens, for instance—it uses an explicit estimator rather than assuming an encoding. An in-process binding may accept a callback; a service uses a negotiated estimator registered on the engine side. If neither exists, that unit budget is unsupported. Guessing a tokenization is how a budget gets exceeded.

### Agent inspection and review context

To support bounded navigation for automated coding agents without forcing full-file dumps or ad-hoc graph traversals, Repin provides high-level inspection and review primitives:

- **`inspectFile`**: Delivers a structured module outline (`SymbolSummary[]`, imports, exports, and ranked recommended entities) without source bodies. When graph data is absent, it degrades to syntax-only or text-only metadata with explicit coverage tracking.
- **Position resolution (`AtPosition`)**: Resolves 1-based source positions or byte offsets to the exact or smallest enclosing entity in the working tree.
- **`reviewContext`**: Composes changed files (or revision diffs), reverse dependency impact, and budgeted context assembly into a single structured evidence bundle with honest omission reporting.

All evidence bodies returned through this funnel are reread directly from current working tree bytes, preserving the rule that the working tree wins.

## 9. Direct retrieval

The working-tree path, available with no index ([Architecture §5](architecture.md#5-direct-retrieval)).

- `files`, `text`, and `regex` need only filesystem access.
- `symbol` needs a pack or a graph; it reports which answered via `source`.
- Ranking uses match quality, path relevance, and artifact class — no graph signals.
- Results share the identical envelope and evidence shape as graph results, so a caller writes one code path.
- Selection and exclusion rules apply identically. Direct retrieval is not a way around them.

### Direct-regex contract

The v1 direct-regex contract is intentionally limited to regular-language
features: literals, character classes and Unicode properties, grouping,
alternation, bounded/unbounded repetition, `^`/`$` anchors, and an explicit
multiline mode. Backreferences, look-around, recursion, replacement
expressions, and implementation-specific execution hooks are not part of the
contract. A provider may implement a larger private syntax, but it MUST reject
features outside this set when serving the public mode.

Compilation and execution are bounded by the request budget and the engine's
resource maxima. The engine reports `INVALID_QUERY` for unsupported syntax or
an over-limit pattern, including the supported feature set and the violated
bound. It MUST NOT silently reinterpret a rejected pattern. Match evidence
uses original byte offsets plus the line/Unicode-scalar positions defined in
[Graph Model §7](graph-model.md#7-positions-and-encoding), with an exclusive
end. Cancellation is checked during compilation and scanning at the F4 safe
points; a timeout or cancellation returns the ordinary bounded result status
and records the reason.

## 10. Quality measurement

"Useful without a model" is not a measurable claim, so it is replaced by one that is.

- A committed labeled query set: queries paired with the entities that should appear in the top N.
- Precision at N tracked over time as a regression guard.
- The set need not be large. It must be stable, committed, and reviewed when it changes — a query set edited to match new behavior measures nothing.
- Ranking changes are evaluated against it before merging.

Detail in [Conformance](conformance.md).
